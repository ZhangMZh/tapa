"""Assemble static Linux LLVM tools with the matching upstream runtimes."""

def _llvm_toolchain_impl(ctx):
    distribution = ctx.path(ctx.attr.distribution).dirname
    static_tools = ctx.os.name == "linux" and ctx.os.arch in ("amd64", "x86_64")
    build = ctx.read(ctx.attr.distribution)
    for entry in distribution.readdir():
        if entry.basename not in ("BUILD.bazel", "MODULE.bazel", "WORKSPACE", "WORKSPACE.bazel", "bin"):
            ctx.symlink(entry, entry.basename)

    if static_tools:
        # Preserve auxiliary LLVM utilities exposed by toolchains_llvm; the
        # static archive replaces every executable used by C/C++ actions.
        tools = ctx.path(ctx.attr.tools).dirname.get_child("bin")
        replacements = {entry.basename: entry for entry in tools.readdir()}
        for entry in distribution.get_child("bin").readdir():
            ctx.symlink(replacements.pop(entry.basename, entry), "bin/" + entry.basename)
        for name, entry in replacements.items():
            ctx.symlink(entry, "bin/" + name)

        # The static archive dispatches every tool through bin/llvm. Declare
        # that binary for each action, including archive and strip actions.
        build = build.replace('srcs = ["bin/', 'srcs = ["bin/llvm", "bin/')
        build = build.replace('        "bin/clang",', '        "bin/llvm",\n        "bin/clang",')
        build = build.replace('        "bin/ld.lld",', '        "bin/llvm",\n        "bin/ld.lld",')
    else:
        ctx.symlink(distribution.get_child("bin"), "bin")
    ctx.file("BUILD.bazel", build)

llvm_toolchain = repository_rule(
    implementation = _llvm_toolchain_impl,
    attrs = {
        "distribution": attr.label(mandatory = True, allow_single_file = True),
        "tools": attr.label(mandatory = True, allow_single_file = True),
    },
)
