//! TAPA XO to SLASH IP-XACT adaptation and V80 hardware linking.

use camino::Utf8PathBuf;
use regex::Regex;
use std::path::Path;

use crate::tools::vitis::timing::KernelTiming;

use crate::error::{Result, XilinxError};
use crate::runtime::process::{ToolInvocation, ToolRunner};
use crate::ToolValue;

#[derive(Debug)]
pub struct SlashLinkJob {
    pub xo: Utf8PathBuf,
    pub config: Utf8PathBuf,
    pub output: Utf8PathBuf,
    pub work_dir: Utf8PathBuf,
    pub slashkit: String,
    pub vivado: String,
    pub jobs: u32,
    pub clock_hz: u64,
    pub floorplan_xdc: Option<Utf8PathBuf>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SlashLinkOutput {
    pub timing_report: Utf8PathBuf,
    pub timing: KernelTiming,
    pub stdout: String,
    pub stderr: String,
}

/// Link a single TAPA kernel for the service shell. A missed target is an
/// error even if SLASH emitted a VBIN with a derated clock.
pub fn link(runner: &dyn ToolRunner, job: &SlashLinkJob) -> Result<SlashLinkOutput> {
    if job.output.extension() != Some("vbin") || job.clock_hz == 0 || job.jobs == 0 {
        return Err(XilinxError::SlashLink(
            "link requires a .vbin output, a positive clock and a positive job count".into(),
        ));
    }
    let xo = crate::util::absolutize(&job.xo);
    let config = crate::util::absolutize(&job.config);
    let output = crate::util::absolutize(&job.output);
    let work = crate::util::absolutize(&job.work_dir);
    let ip = work.join("slash-ip");
    let slashkit = resolve_executable(&job.slashkit)?;
    let vivado = resolve_executable(&job.vivado)?;
    if output.exists() || ip.exists() {
        return Err(XilinxError::SlashLink(
            "SLASH output or export directory already exists; use a fresh link work directory"
                .into(),
        ));
    }
    fs_err::create_dir_all(&work)?;
    let adapter = work.join("slash_export.py");
    fs_err::write(&adapter, include_str!("export.py"))?;
    let mut export = ToolInvocation::new("python3")
        .arg(ToolValue::path(adapter.clone()))
        .arg("export")
        .arg(ToolValue::path(xo.clone()))
        .arg(ToolValue::path(ip.clone()));
    export.uploads = vec![adapter.clone(), xo];
    export.downloads = vec![ip.clone()];
    run(runner, &export)?;

    let mut invocation = ToolInvocation::new(slashkit.as_str())
        .arg("link")
        .arg("--vivado")
        .arg(vivado.as_str())
        .arg("--jobs")
        .arg(job.jobs.to_string())
        .arg("-c")
        .arg(ToolValue::path(config.clone()))
        .arg("-k")
        .arg(ToolValue::path(ip.join("impl/ip/component.xml")))
        .arg("-o")
        .arg(ToolValue::path(output.clone()))
        .arg("-p")
        .arg("hw")
        .arg("--clock-hz")
        .arg(job.clock_hz.to_string());
    invocation.uploads = vec![ip, config];
    // The implementation project is retained on the tool host. Verification
    // runs there too, so remote linking need not download the Vivado project.
    invocation.downloads = vec![output.clone(), Utf8PathBuf::from(format!("{output}.prj"))];
    if let Some(xdc) = job.floorplan_xdc.as_ref() {
        let xdc = crate::util::absolutize(xdc);
        let hook = work.join("slash_floorplan.tcl");
        fs_err::write(&hook, pre_synth_hook(&xdc))?;
        invocation
            .args
            .extend(["--pre-synth-tcls".into(), ToolValue::path(hook.clone())]);
        invocation.uploads.extend([hook, xdc]);
    }
    let link_output = run_output(runner, &invocation)?;

    // SLASH owns both the hardware address assignment and the system map.
    // Verify that they agree without rewriting either artifact.
    let project = Utf8PathBuf::from(format!("{output}.prj"));
    let mut verify = ToolInvocation::new("python3")
        .arg(ToolValue::path(adapter.clone()))
        .arg("verify")
        .arg(ToolValue::path(output.clone()))
        .arg(job.clock_hz.to_string());
    verify.uploads = vec![adapter, output.clone(), project];
    run(runner, &verify)?;
    let timing_report = find_timing_report(&Utf8PathBuf::from(format!("{output}.prj")))?;
    let timing_text = fs_err::read_to_string(&timing_report)?;
    let timing = parse_timing(&timing_text, job.clock_hz)?;
    Ok(SlashLinkOutput {
        timing_report,
        timing,
        stdout: link_output.stdout,
        stderr: link_output.stderr,
    })
}

fn pre_synth_hook(floorplan_xdc: &Utf8PathBuf) -> String {
    // Floorplanning needs the linked RM before placement. It is Tcl, not a
    // plain XDC file, and must not modify SLASH's control address assignment.
    format!(
        "set_property STEPS.PLACE_DESIGN.TCL.PRE [list {{{floorplan_xdc}}}] [get_runs impl_1]\n"
    )
}

fn resolve_executable(program: &str) -> Result<Utf8PathBuf> {
    let path = Path::new(program);
    if path.is_absolute() || program.contains(std::path::MAIN_SEPARATOR) {
        let path = crate::util::absolutize(camino::Utf8Path::new(program));
        if path.is_file() {
            return Ok(path);
        }
    } else if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join(program);
            if candidate.is_file() {
                return Ok(crate::util::utf8(candidate));
            }
        }
    }
    Err(XilinxError::SlashLink(format!(
        "SLASH tool `{program}` is not an executable file or PATH entry"
    )))
}

fn run(runner: &dyn ToolRunner, invocation: &ToolInvocation) -> Result<()> {
    let output = run_output(runner, invocation)?;
    if output.exit_code != 0 {
        return Err(super::tool_failure(&invocation.program, output));
    }
    Ok(())
}

fn run_output(
    runner: &dyn ToolRunner,
    invocation: &ToolInvocation,
) -> Result<crate::runtime::process::ToolOutput> {
    let output = runner.run(invocation)?;
    if output.exit_code != 0 {
        return Err(super::tool_failure(&invocation.program, output));
    }
    Ok(output)
}

fn find_timing_report(project: &Utf8PathBuf) -> Result<Utf8PathBuf> {
    let root = Path::new(project.as_str()).join("slash_rm");
    let mut reports = fs_err::read_dir(&root)?
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("report_timing_") && name.ends_with(".txt"))
        })
        .map(crate::util::utf8)
        .collect::<Vec<_>>();
    reports.sort();
    match reports.as_slice() {
        [report] => Ok(report.clone()),
        [] => Err(XilinxError::SlashLink(format!(
            "SLASH produced no routed timing report under `{project}`"
        ))),
        _ => Err(XilinxError::SlashLink(format!(
            "SLASH produced multiple routed timing reports under `{project}`"
        ))),
    }
}

fn parse_timing(report: &str, clock_hz: u64) -> Result<KernelTiming> {
    let requested_mhz = clock_hz / 1_000_000;
    if requested_mhz == 0 || requested_mhz > u64::from(u32::MAX) {
        return Err(XilinxError::InvalidFrequency(format!(
            "invalid SLASH clock `{clock_hz}` Hz"
        )));
    }
    let requested_mhz = f64::from(u32::try_from(requested_mhz).expect("checked above"));
    let row = Regex::new(
        r"(?m)WNS\(ns\)\s+TNS\(ns\).*?\n\s*-+[^\n]*\n\s*([-+0-9.]+)\s+([-+0-9.]+)\s+\d+\s+\d+\s+([-+0-9.]+)\s+([-+0-9.]+)",
    )
    .expect("static SLASH timing expression");
    let captures = row.captures(report).ok_or_else(|| {
        XilinxError::TimingSummaryParse(
            "SLASH routed timing report has no WNS/TNS/WHS/THS summary".into(),
        )
    })?;
    let parse = |index: usize, name: &str| -> Result<f64> {
        let value = captures[index]
            .parse::<f64>()
            .map_err(|_| XilinxError::TimingSummaryParse(format!("invalid SLASH {name} value")))?;
        if value.is_finite() {
            Ok(value)
        } else {
            Err(XilinxError::TimingSummaryParse(format!(
                "SLASH {name} value is not finite"
            )))
        }
    };
    let wns = parse(1, "WNS")?;
    let tns = parse(2, "TNS")?;
    let whs = parse(3, "WHS")?;
    let ths = parse(4, "THS")?;
    if [wns, tns, whs, ths].iter().any(|value| *value < 0.0) {
        return Err(XilinxError::TimingSummaryParse(format!(
            "SLASH timing failed at {requested_mhz:.0} MHz: WNS={wns}, TNS={tns}, WHS={whs}, THS={ths}"
        )));
    }
    let period = 1000.0 / requested_mhz;
    let achieved_period = period - wns;
    if achieved_period <= 0.0 {
        return Err(XilinxError::TimingSummaryParse(
            "SLASH achieved period is not positive".into(),
        ));
    }
    Ok(KernelTiming {
        reported_target_period_ns: period,
        reported_target_mhz: requested_mhz,
        wns_ns: wns,
        achieved_period_ns: achieved_period,
        fmax_mhz: 1000.0 / achieved_period,
    })
}

#[cfg(test)]
mod tests {
    use super::{link, parse_timing, pre_synth_hook, resolve_executable, SlashLinkJob};
    use crate::runtime::process::{MockToolRunner, ToolOutput};
    use camino::Utf8PathBuf;

    #[test]
    fn slash_invocations_relocate_artifacts_but_preserve_tool_host_literals() {
        let dir = tempfile::tempdir().expect("temporary work directory");
        let root = crate::util::utf8(dir.path()).join("work with spaces");
        let runner = MockToolRunner::new();
        runner.push_ok("python3", ToolOutput::default());
        runner.push_ok("/bin/sh", ToolOutput::default());
        // Capture all invocations without pretending to produce a valid VBIN
        // or routed timing report. No external tool is executed by this test.
        runner.push_failure("python3", 1, "stop after invocation capture");
        let job = SlashLinkJob {
            xo: root.join("kernel.xo"),
            config: root.join("connectivity.ini"),
            output: root.join("kernel.vbin"),
            work_dir: root.join("link"),
            slashkit: "/bin/sh".into(),
            vivado: "/bin/sh".into(),
            jobs: 2,
            clock_hz: 100_000_000,
            floorplan_xdc: Some(root.join("floorplan.xdc")),
        };
        let error = link(&runner, &job).expect_err("mock verification must stop the link");
        assert!(error.to_string().contains("stop after invocation capture"));

        let calls = runner.calls();
        assert_eq!(calls.len(), 3, "export, link, and verify");
        for invocation in &calls {
            for argument in &invocation.args {
                let local = argument.to_string();
                let remote = argument.render(|path| format!("/remote{path}"));
                if local.starts_with(root.as_str()) {
                    assert_eq!(remote, format!("/remote{local}"), "artifact: {local}");
                } else {
                    assert_eq!(remote, local, "tool-host path or option must stay literal");
                }
            }
        }
        let link_args = calls[1].local_args();
        assert!(link_args
            .windows(2)
            .any(|pair| pair == ["--vivado", "/bin/sh"]));
        assert!(link_args.windows(2).any(|pair| pair == ["-p", "hw"]));
        assert!(link_args
            .windows(2)
            .any(|pair| pair == ["--clock-hz", "100000000"]));
    }

    #[test]
    fn v80_pre_synth_hook_only_sets_floorplan() {
        let hook = pre_synth_hook(&Utf8PathBuf::from("/tmp/floorplan.xdc"));
        assert_eq!(
            hook,
            "set_property STEPS.PLACE_DESIGN.TCL.PRE [list {/tmp/floorplan.xdc}] [get_runs impl_1]\n"
        );
    }

    #[test]
    fn parses_vivado_timing_columns_and_rejects_violations() {
        let report =
            "    WNS(ns) TNS(ns) TNS Failing Endpoints TNS Total Endpoints WHS(ns) THS(ns)\n\
                      ------- ------- --------------------- ------------------- ------- -------\n\
                        0.066   0.000                     0               87110   0.022   0.000\n";
        let timing = parse_timing(report, 100_000_000).expect("Vivado column separator");
        assert!((timing.wns_ns - 0.066).abs() < 1e-9);
        assert!((timing.reported_target_period_ns - 10.0).abs() < 1e-9);
        for value in ["0.066", "0.022"] {
            let failing = report.replace(value, &format!("-{value}"));
            let error = parse_timing(&failing, 100_000_000).expect_err("timing violation");
            assert!(error.to_string().contains("SLASH timing failed"));
        }
    }

    #[test]
    fn resolves_absolute_executable() {
        let resolved = resolve_executable("/bin/sh").expect("shell must exist");
        assert_eq!(resolved, "/bin/sh");
    }

    #[test]
    fn rejects_missing_executable() {
        let error = resolve_executable("tapa-slash-missing-tool").expect_err("must fail");
        assert!(error.to_string().contains("not an executable file"));
    }
}
