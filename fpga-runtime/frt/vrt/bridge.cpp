#include <vrt/buffer.hpp>
#include <vrt/device.hpp>
#include <vrt/kernel.hpp>

#include <chrono>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <map>
#include <memory>
#include <optional>
#include <sstream>
#include <stdexcept>
#include <string>
#include <thread>
#include <vector>

namespace {

using Clock = std::chrono::steady_clock;

uint64_t Elapsed(Clock::time_point start) {
  return std::chrono::duration_cast<std::chrono::nanoseconds>(Clock::now() -
                                                              start)
      .count();
}

thread_local std::string last_error;

template <typename F>
int Check(F&& operation) noexcept {
  try {
    last_error.clear();
    operation();
    return 0;
  } catch (const std::exception& error) {
    last_error = error.what();
  } catch (...) {
    last_error = "unknown VRT exception";
  }
  return -1;
}

struct Buffer {
  std::unique_ptr<vrt::Buffer<uint8_t>> device;
  uint8_t* host;
  size_t size;
  int access;
};

struct Instance {
  vrt::Device device;
  vrt::Kernel kernel;
  std::map<uint32_t, Buffer> buffers;
  std::vector<std::optional<uint64_t>> arguments;
  std::string failure;
  bool launched = false;
  bool finished = false;
  Clock::time_point start;
  uint64_t load_ns = 0;
  uint64_t compute_ns = 0;
  uint64_t store_ns = 0;

  Instance(const char* path, const char* bdf, const char* name)
      : device(bdf, path),
        kernel(device, name),
        arguments(kernel.getFunctionalArgs().size()) {
    if (device.getPlatform() != vrt::Platform::HARDWARE) {
      throw std::runtime_error("FRT VRT backend requires a hardware VBIN");
    }
    ReadControl();
  }

  const vrt::FunctionalArg& Arg(uint32_t index) {
    if (launched && !finished) {
      throw std::runtime_error("cannot change arguments while VRT is running");
    }
    const auto& args = kernel.getFunctionalArgs();
    if (index >= args.size() || args[index].idx != index) {
      throw std::runtime_error("VRT argument index is missing or out of order");
    }
    return args[index];
  }

  [[noreturn]] void Fail(const std::string& message) {
    failure = message;
    throw std::runtime_error(failure);
  }

  uint32_t ReadControl() {
    const uint32_t control = kernel.read(0);
    if (control == UINT32_MAX) {
      std::ostringstream message;
      message << "VRT kernel " << kernel.getName()
              << " control read returned 0xffffffff at AXI base 0x" << std::hex
              << kernel.getPhysAddr()
              << "; check VBIN metadata against the implemented address map";
      Fail(message.str());
    }
    return control;
  }

  void WriteArguments() {
    for (size_t index = 0; index < arguments.size(); ++index) {
      const auto& arg = kernel.getFunctionalArgs()[index];
      if (!arguments[index]) {
        throw std::runtime_error("unbound VRT kernel argument " + arg.name);
      }
    }
    for (size_t index = 0; index < arguments.size(); ++index) {
      const auto& arg = kernel.getFunctionalArgs()[index];
      const uint64_t value = *arguments[index];
      for (uint32_t word = 0; word < arg.range / 32; ++word) {
        const uint32_t offset = arg.offset + word * 4;
        const uint32_t expected = static_cast<uint32_t>(value >> (word * 32));
        kernel.write(offset, expected);
        if (arg.readable && kernel.read(offset) != expected) {
          std::ostringstream message;
          message << "VRT argument " << arg.name
                  << " readback mismatch at offset 0x" << std::hex << offset
                  << "; kernel was not started";
          Fail(message.str());
        }
      }
    }
  }

  bool Poll() {
    if (!failure.empty()) throw std::runtime_error(failure);
    if (launched && !finished) {
      const uint32_t control = ReadControl();
      if (control & 2u) {
        finished = true;
        compute_ns = Elapsed(start);
      } else if (Clock::now() - start >= std::chrono::minutes(5)) {
        Fail(
            "VRT kernel completion timed out after 300 seconds; device buffers "
            "retained");
      }
    }
    return finished;
  }

  void Wait() {
    if (!launched) return;
    while (!Poll()) std::this_thread::sleep_for(std::chrono::microseconds(50));
  }
};

}  // namespace

// Only C types cross the Rust/VRT boundary. The adapter links to the installed
// VRT C++ ABI, while software simulation and XRT hosts never load it.
extern "C" {

const char* frt_vrt_error() { return last_error.c_str(); }

void* frt_vrt_open(const char* path, const char* bdf, const char* kernel) {
  Instance* instance = nullptr;
  Check([&] { instance = new Instance(path, bdf, kernel); });
  return instance;
}

int frt_vrt_close(void* handle) {
  return Check([&] {
    auto* instance = static_cast<Instance*>(handle);
    // VRT has no kernel cancellation operation. Keep all allocations alive
    // until the device has stopped accessing them, including on early close.
    instance->Wait();
    delete instance;
  });
}

int frt_vrt_scalar(void* handle, uint32_t index, const uint8_t* data,
                   size_t size) {
  return Check([&] {
    auto& instance = *static_cast<Instance*>(handle);
    const auto& arg = instance.Arg(index);
    if (!arg.port.empty() || (size != 4 && size != 8) ||
        arg.range != size * 8) {
      throw std::runtime_error("VRT scalar argument kind or width mismatch");
    }
    uint64_t value = 0;
    std::memcpy(&value, data, size);
    instance.arguments[index] = value;
  });
}

int frt_vrt_buffer(void* handle, uint32_t index, uint8_t* host, size_t size,
                   int access) {
  return Check([&] {
    auto& instance = *static_cast<Instance*>(handle);
    const auto& arg = instance.Arg(index);
    if (arg.port.empty() || arg.range != 64 || size == 0 || host == nullptr ||
        access < 0 || access > 3) {
      throw std::runtime_error("invalid VRT buffer argument");
    }
    auto buffer = std::make_unique<vrt::Buffer<uint8_t>>(
        instance.device, size, instance.kernel.argMemoryConfig(arg.name));
    instance.arguments[index] = buffer->getPhysAddr();
    instance.buffers.insert_or_assign(
        index, Buffer{std::move(buffer), host, size, access});
  });
}

int frt_vrt_write(void* handle) {
  return Check([&] {
    auto& instance = *static_cast<Instance*>(handle);
    if (instance.launched && !instance.finished) {
      throw std::runtime_error("cannot upload buffers while VRT is running");
    }
    const auto start = Clock::now();
    for (auto& [index, buffer] : instance.buffers) {
      if (buffer.access == 1 || buffer.access == 3) {
        std::memcpy(buffer.device->get(), buffer.host, buffer.size);
        buffer.device->sync(vrt::SyncType::HOST_TO_DEVICE);
      }
    }
    instance.load_ns = Elapsed(start);
  });
}

int frt_vrt_read(void* handle) {
  return Check([&] {
    auto& instance = *static_cast<Instance*>(handle);
    instance.Wait();
    const auto start = Clock::now();
    for (auto& [index, buffer] : instance.buffers) {
      if (buffer.access == 2 || buffer.access == 3) {
        buffer.device->sync(vrt::SyncType::DEVICE_TO_HOST);
        std::memcpy(buffer.host, buffer.device->get(), buffer.size);
      }
    }
    instance.store_ns = Elapsed(start);
  });
}

int frt_vrt_exec(void* handle) {
  return Check([&] {
    auto& instance = *static_cast<Instance*>(handle);
    if (instance.launched && !instance.finished) {
      throw std::runtime_error("VRT kernel is already running");
    }
    if (!instance.failure.empty()) throw std::runtime_error(instance.failure);
    // Read/COR consumes an old ap_done before a new launch. Check accessibility
    // before writing any parameter, and verify parameters before asserting
    // start.
    instance.ReadControl();
    instance.WriteArguments();
    instance.start = Clock::now();
    instance.launched = true;
    instance.finished = false;
    instance.kernel.write(0, 1u);
  });
}

int frt_vrt_finish(void* handle) {
  return Check([&] { static_cast<Instance*>(handle)->Wait(); });
}

int frt_vrt_poll(void* handle) {
  bool finished = false;
  if (Check([&] { finished = static_cast<Instance*>(handle)->Poll(); }) != 0)
    return -1;
  return finished ? 1 : 0;
}

uint64_t frt_vrt_load_ns(void* handle) {
  return static_cast<Instance*>(handle)->load_ns;
}
uint64_t frt_vrt_compute_ns(void* handle) {
  return static_cast<Instance*>(handle)->compute_ns;
}
uint64_t frt_vrt_store_ns(void* handle) {
  return static_cast<Instance*>(handle)->store_ns;
}

}  // extern "C"
