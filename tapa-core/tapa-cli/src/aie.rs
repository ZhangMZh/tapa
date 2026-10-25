//! Shared AI Engine build inputs for `tapa synth` and `tapa aiesim`: where
//! each kernel's task is defined in the rewritten tree, the kernel include
//! paths and definitions, and the device `aiecompiler` targets.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use camino::{Utf8Path, Utf8PathBuf};
use tapa_ir::{AiePartition, Graph};
use tapa_xilinx::tools::aie::graph::{render_graph, GraphOptions, KernelSource, SimMain};
use tapa_xilinx::tools::aie::{AieCompileJob, AieDevice, AieTarget};

use crate::error::{CliError, Result};
use crate::tapacc::discover::find_resource;

/// The device an AI Engine graph compiles for: a platform's `.xpfm`, or a
/// part number when no platform is known.
pub fn device(platform: Option<&str>, part: Option<&str>) -> Result<AieDevice> {
    if let Some(platform) = platform {
        let dir = crate::steps::synth::resolve_platform_dir(platform).ok_or_else(|| {
            CliError::InvalidArg(format!("cannot find the platform `{platform}`"))
        })?;
        let xpfm = fs::read_dir(&dir)?
            .filter_map(std::result::Result::ok)
            .map(|entry| crate::util::utf8(entry.path()))
            .find(|path| path.extension() == Some("xpfm"))
            .ok_or_else(|| {
                CliError::InvalidArg(format!("platform `{platform}` has no `.xpfm` in {dir}"))
            })?;
        return Ok(AieDevice::Platform(xpfm));
    }
    part.map(|part| AieDevice::Part(part.to_string()))
        .ok_or_else(|| {
            CliError::InvalidArg(
                "the AI Engine compiler needs a device; pass `--platform` or `--part-num`, \
                 or run `tapa synth` first"
                    .to_string(),
            )
        })
}

/// Where each AI Engine task is defined: the first translation unit of its
/// manifest whose rewritten text carries its guard, else the first one (a
/// task defined in a shared header is guarded in that header).
fn kernel_sources(
    tree: &Utf8Path,
    graph: &Graph,
    partition: &AiePartition,
) -> Result<BTreeMap<String, KernelSource>> {
    let mut sources = BTreeMap::new();
    for kernel in &partition.kernels {
        let task = &graph.tasks[&kernel.task];
        let mut chosen = task.srcs.first().cloned();
        for src in &task.srcs {
            let text = fs::read_to_string(tree.join(src))?;
            if task
                .defines
                .iter()
                .any(|define| text.contains(define.as_str()))
            {
                chosen = Some(src.clone());
                break;
            }
        }
        let tu = chosen.ok_or_else(|| {
            CliError::Codegen(format!(
                "task `{}` has an empty `srcs` manifest",
                kernel.task
            ))
        })?;
        sources.insert(
            kernel.task.clone(),
            KernelSource {
                tu,
                defines: task.defines.clone(),
            },
        );
    }
    Ok(sources)
}

/// One graph build: where, for which target and device, at which PL clock,
/// and with which simulator entry point.
pub struct GraphBuild {
    pub dir: Utf8PathBuf,
    pub target: AieTarget,
    pub device: AieDevice,
    pub pl_freq_mhz: Option<f64>,
    pub sim: Option<SimMain>,
}

/// Writes the graph sources of `partition` into the build directory (wiped
/// first) and returns the compile job for it.
pub fn prepare(
    work_dir: &Path,
    graph: &Graph,
    partition: &AiePartition,
    build: GraphBuild,
) -> Result<AieCompileJob> {
    let GraphBuild {
        dir,
        target,
        device,
        pl_freq_mhz,
        sim,
    } = build;
    let dir = dir.as_path();
    let tree = crate::util::utf8(work_dir.join(crate::tapacc::REWRITTEN_DIR));
    let options = GraphOptions {
        sources: kernel_sources(&tree, graph, partition)?,
        plio_mhz: pl_freq_mhz,
        sim,
    };
    let files = render_graph(partition, &options)?;
    if dir.exists() {
        fs::remove_dir_all(dir)?;
    }
    for (name, text) in files {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, text)?;
    }
    fs::create_dir_all(dir.join("data"))?;

    // Kernels compile the rewritten tree with the AI Engine target of
    // `tapa.h`; the user's include paths and definitions carry over.
    let mut include_dirs = vec![tree.clone()];
    let mut seen = std::collections::BTreeSet::new();
    for task in graph.tasks.values() {
        for include in &task.include_dirs {
            if !include.is_empty() && seen.insert(include.clone()) {
                include_dirs.push(tree.join(include));
            }
        }
    }
    let mut defines = vec![
        "TAPA_TARGET_DEVICE_".to_string(),
        "TAPA_TARGET_AIE_".to_string(),
    ];
    let mut flags = graph.cflags.iter();
    while let Some(flag) = flags.next() {
        for (prefix, out) in [("-I", true), ("-isystem", true), ("-D", false)] {
            let Some(rest) = flag.strip_prefix(prefix) else {
                continue;
            };
            let value = if rest.is_empty() {
                flags.next().cloned().unwrap_or_default()
            } else {
                rest.to_string()
            };
            if out {
                include_dirs.push(crate::util::utf8(std::path::absolute(&value)?));
            } else {
                defines.push(value);
            }
            break;
        }
    }
    if let Ok(lib) = find_resource("tapa-lib-include") {
        include_dirs.push(crate::util::utf8(lib));
    }
    Ok(AieCompileJob::builder()
        .graph_dir(dir.to_path_buf())
        .target(target)
        .device(device)
        .include_dirs(include_dirs)
        .defines(defines)
        .pl_freq_mhz(pl_freq_mhz)
        .build())
}

/// The PL clock in MHz a synthesized design runs its PLIOs at.
pub fn pl_freq_mhz(state: &tapa_ir::WorkState) -> Option<f64> {
    state
        .flow
        .clock_period
        .map(|period| 1000.0 / period.nanoseconds())
}

/// `<work_dir>/aie/<name>`.
pub fn dir(work_dir: &Path, name: &str) -> Utf8PathBuf {
    crate::util::utf8(work_dir.join("aie").join(name))
}
