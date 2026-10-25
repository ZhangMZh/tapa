//! Mapping and program-memory reports from an `aiecompiler` graph build.
//!
//! `compiler_report.json` is used instead of the mapping-analysis text: it
//! carries the same placements as numbers and directly associates PLIO
//! logical names and directions with block instance IDs. Program sizes come
//! from `report_pm.txt`, whose header declares byte units.

use std::collections::{BTreeMap, BTreeSet};

use camino::Utf8Path;
use serde::{Deserialize, Serialize};

use crate::error::{Result, XilinxError};

/// The AI Engine resources assigned to a compiled graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AieReport {
    /// Number of distinct core tiles used by AIE kernels.
    pub tiles_used: usize,
    /// Logical PLIO name → its interface placement, ordered by name.
    pub plio_map: BTreeMap<String, AiePlioReport>,
    /// ADF kernel function name → its placement and memory use, ordered by
    /// name.
    pub per_kernel: BTreeMap<String, AieKernelReport>,
}

/// An AI Engine core tile coordinate (`CR(column,row)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct AieTile {
    pub column: u32,
    pub row: u32,
}

/// A PLIO's interface placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AiePlioReport {
    /// Interface column (`IO(column)`).
    pub column: u32,
    pub direction: PlioDirection,
}

/// PLIO direction from the graph's perspective.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PlioDirection {
    /// PL → AIE (`pl2aie` in the compiler report).
    Input,
    /// AIE → PL (`aie2pl` in the compiler report).
    Output,
}

/// An AIE kernel instance's placement and memory use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct AieKernelReport {
    pub tile: AieTile,
    /// Used program memory in bytes, from `PM Size Used` for this tile.
    pub program_bytes: u64,
    /// Per-kernel data memory in bytes. The supported compiler reports do
    /// not define this unambiguously, so this is currently always `None`;
    /// stack, heap, stream buffers, and system memory are not substitutes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_bytes: Option<u64>,
}

/// Read the mapping report from `graph_dir`, which must contain `Work/`.
pub fn read_report(graph_dir: &Utf8Path) -> Result<AieReport> {
    let reports = graph_dir.join("Work").join("reports");
    let compiler_path = reports.join("compiler_report.json");
    let compiler_text = read_text(&compiler_path)?;
    let compiler: CompilerReport =
        serde_json::from_str(&compiler_text).map_err(|source| XilinxError::AieReportJson {
            path: compiler_path.clone(),
            source,
        })?;
    let (tiles, plio_map, kernels) =
        parse_compiler(compiler).map_err(|detail| XilinxError::AieReportParse {
            path: compiler_path,
            detail,
        })?;

    let pm_path = reports.join("report_pm.txt");
    let pm_text = read_text(&pm_path)?;
    let program_sizes =
        parse_program_sizes(&pm_text).map_err(|detail| XilinxError::AieReportParse {
            path: pm_path.clone(),
            detail,
        })?;
    let mut per_kernel = BTreeMap::new();
    for kernel in kernels {
        let program_bytes =
            *program_sizes
                .get(&kernel.tile)
                .ok_or_else(|| XilinxError::AieReportParse {
                    path: pm_path.clone(),
                    detail: format!(
                        "no PM Size Used for kernel `{}` at CR({},{})",
                        kernel.name, kernel.tile.column, kernel.tile.row
                    ),
                })?;
        per_kernel.insert(
            kernel.name,
            AieKernelReport {
                tile: kernel.tile,
                program_bytes,
                data_bytes: None,
            },
        );
    }
    Ok(AieReport {
        tiles_used: tiles.len(),
        plio_map,
        per_kernel,
    })
}

fn read_text(path: &Utf8Path) -> Result<String> {
    fs_err::read_to_string(path).map_err(|source| XilinxError::AieReportIo {
        path: path.to_path_buf(),
        source,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompilerReport {
    block_types: BTreeMap<String, BlockType>,
    block_instances: BTreeMap<String, BlockInstance>,
    mapping: Mapping,
}

#[derive(Deserialize)]
struct BlockType {
    #[serde(rename = "type")]
    kind: String,
    function: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BlockInstance {
    block_type: String,
    logical_name: Option<String>,
    fabric: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Mapping {
    block_instance_mapping: BTreeMap<String, BlockMapping>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BlockMapping {
    core_info: Option<CoreInfo>,
    plio_info: Option<PlioInfo>,
}

#[derive(Deserialize)]
struct CoreInfo {
    column: u32,
    row: u32,
}

#[derive(Deserialize)]
struct PlioInfo {
    column: u32,
    direction: String,
}

struct KernelPlacement {
    name: String,
    tile: AieTile,
}

type CompilerPieces = (
    BTreeSet<AieTile>,
    BTreeMap<String, AiePlioReport>,
    Vec<KernelPlacement>,
);

fn parse_compiler(report: CompilerReport) -> std::result::Result<CompilerPieces, String> {
    let mut tiles = BTreeSet::new();
    let mut kernel_tiles = BTreeMap::new();
    let mut kernels = Vec::new();
    let mut kernel_names = BTreeSet::new();
    let mut plios = BTreeMap::new();
    for (id, instance) in report.block_instances {
        let block_type = report
            .block_types
            .get(&instance.block_type)
            .ok_or_else(|| {
                format!(
                    "block instance `{id}` refers to missing block type `{}`",
                    instance.block_type
                )
            })?;
        match block_type.kind.as_str() {
            "function" => {
                match instance.fabric.as_deref() {
                    Some("me") => {}
                    Some("fpga") => continue,
                    fabric => {
                        return Err(format!(
                            "function instance `{id}` has unknown or missing fabric `{fabric:?}`"
                        ))
                    }
                }
                let name = required_name(block_type.function.clone(), "function", &id)?;
                let mapping = report
                    .mapping
                    .block_instance_mapping
                    .get(&id)
                    .ok_or_else(|| format!("no blockInstanceMapping for kernel instance `{id}`"))?;
                let core = mapping
                    .core_info
                    .as_ref()
                    .ok_or_else(|| format!("no coreInfo for kernel instance `{id}` (`{name}`)"))?;
                let tile = AieTile {
                    column: core.column,
                    row: core.row,
                };
                if let Some(previous) = kernel_tiles.insert(tile, name.clone()) {
                    return Err(format!(
                        "kernels `{previous}` and `{name}` share CR({},{}); per-kernel program \
                         memory is ambiguous",
                        tile.column, tile.row
                    ));
                }
                if !kernel_names.insert(name.clone()) {
                    return Err(format!("duplicate ADF kernel function name `{name}`"));
                }
                tiles.insert(tile);
                kernels.push(KernelPlacement { name, tile });
            }
            "plio" => {
                let name = required_name(instance.logical_name, "logicalName", &id)?;
                let mapping = report
                    .mapping
                    .block_instance_mapping
                    .get(&id)
                    .ok_or_else(|| format!("no blockInstanceMapping for PLIO instance `{id}`"))?;
                let info = mapping
                    .plio_info
                    .as_ref()
                    .ok_or_else(|| format!("no plioInfo for PLIO instance `{id}` (`{name}`)"))?;
                let direction = match info.direction.as_str() {
                    "pl2aie" => PlioDirection::Input,
                    "aie2pl" => PlioDirection::Output,
                    direction => {
                        return Err(format!("PLIO `{name}` has unknown direction `{direction}`"))
                    }
                };
                let plio = AiePlioReport {
                    column: info.column,
                    direction,
                };
                if plios.insert(name.clone(), plio).is_some() {
                    return Err(format!("duplicate logical PLIO name `{name}`"));
                }
            }
            _ => {}
        }
    }
    Ok((tiles, plios, kernels))
}

fn required_name(
    name: Option<String>,
    field: &str,
    id: &str,
) -> std::result::Result<String, String> {
    match name {
        Some(name) if !name.is_empty() => Ok(name),
        _ => Err(format!("block instance `{id}` has no non-empty {field}")),
    }
}

fn parse_program_sizes(text: &str) -> std::result::Result<BTreeMap<AieTile, u64>, String> {
    if !text
        .lines()
        .any(|line| line.trim() == "Note: All sizes are in bytes")
    {
        return Err("missing `Note: All sizes are in bytes` unit declaration".into());
    }
    let mut sizes = BTreeMap::new();
    let mut current: Option<(AieTile, Option<u64>)> = None;
    for line in text.lines().map(str::trim) {
        if let Some(coordinate) = line.strip_prefix("Core ") {
            finish_core(&mut current, &mut sizes)?;
            current = Some((parse_core(coordinate)?, None));
        } else if let Some(value) = line.strip_prefix("PM Size Used = ") {
            let (tile, used) = current
                .as_mut()
                .ok_or_else(|| "PM Size Used appears before a Core heading".to_string())?;
            if used.is_some() {
                return Err(format!(
                    "duplicate PM Size Used for CR({},{})",
                    tile.column, tile.row
                ));
            }
            *used = Some(
                value
                    .parse::<u64>()
                    .map_err(|error| format!("invalid PM Size Used `{value}`: {error}"))?,
            );
        }
    }
    finish_core(&mut current, &mut sizes)?;
    Ok(sizes)
}

fn parse_core(coordinate: &str) -> std::result::Result<AieTile, String> {
    let (column, row) = coordinate
        .split_once('_')
        .ok_or_else(|| format!("invalid Core coordinate `{coordinate}`; expected `column_row`"))?;
    if row.contains('_') {
        return Err(format!(
            "invalid Core coordinate `{coordinate}`; expected `column_row`"
        ));
    }
    let parse = |value: &str| {
        value
            .parse::<u32>()
            .map_err(|error| format!("invalid Core coordinate `{coordinate}`: `{value}`: {error}"))
    };
    Ok(AieTile {
        column: parse(column)?,
        row: parse(row)?,
    })
}

fn finish_core(
    current: &mut Option<(AieTile, Option<u64>)>,
    sizes: &mut BTreeMap<AieTile, u64>,
) -> std::result::Result<(), String> {
    let Some((tile, used)) = current.take() else {
        return Ok(());
    };
    let used = used.ok_or_else(|| {
        format!(
            "Core heading CR({},{}) has no PM Size Used",
            tile.column, tile.row
        )
    })?;
    if sizes.insert(tile, used).is_some() {
        return Err(format!("duplicate Core CR({},{})", tile.column, tile.row));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMPILER: &str = r#"{
      "blockTypes": {
        "z": {"type": "function", "function": "tapa_aie_Z_0"},
        "a": {"type": "function", "function": "tapa_aie_Add_0"},
        "in": {"type": "plio"},
        "out": {"type": "plio"},
        "ps": {"type": "function", "function": "ps_main"}
      },
      "blockInstances": {
        "i9": {"blockType": "z", "fabric": "me"},
        "i8": {"blockType": "out", "logicalName": "c_q"},
        "i0": {"blockType": "a", "fabric": "me"},
        "i7": {"blockType": "in", "logicalName": "a_q"},
        "ps_i10": {"blockType": "ps", "fabric": "fpga"}
      },
      "mapping": {"blockInstanceMapping": {
        "i9": {"coreInfo": {"column": 25, "row": 1}},
        "i8": {"plioInfo": {"column": 24, "direction": "aie2pl"}},
        "i0": {"coreInfo": {"column": 23, "row": 0}},
        "i7": {"plioInfo": {"column": 26, "direction": "pl2aie"}}
      }}
    }"#;

    const PM: &str = "Note: All sizes are in bytes\n\n\
                      Core 25_1\n\tPM Size Used = 11668\n\
                      Core 23_0\n\tPM Size Used = 0\n";

    fn write_reports(compiler: &str, pm: Option<&str>) -> tempfile::TempDir {
        let temp = tempfile::tempdir().expect("temporary graph directory");
        let reports = temp.path().join("Work/reports");
        fs_err::create_dir_all(&reports).expect("report directory");
        fs_err::write(reports.join("compiler_report.json"), compiler).expect("compiler report");
        if let Some(pm) = pm {
            fs_err::write(reports.join("report_pm.txt"), pm).expect("program-memory report");
        }
        temp
    }

    fn graph_dir(temp: &tempfile::TempDir) -> &Utf8Path {
        Utf8Path::from_path(temp.path()).expect("UTF-8 temporary path")
    }

    #[test]
    fn report_associates_names_tiles_directions_and_bytes_deterministically() {
        let temp = write_reports(COMPILER, Some(PM));
        let report = read_report(graph_dir(&temp)).expect("parse report");
        assert_eq!(report.tiles_used, 2, "two distinct core tiles");
        assert_eq!(
            report
                .plio_map
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["a_q", "c_q"],
            "logical PLIO names are sorted"
        );
        assert_eq!(
            report.plio_map["a_q"],
            AiePlioReport {
                column: 26,
                direction: PlioDirection::Input,
            }
        );
        assert_eq!(
            report.plio_map["c_q"],
            AiePlioReport {
                column: 24,
                direction: PlioDirection::Output,
            }
        );
        assert_eq!(
            report
                .per_kernel
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["tapa_aie_Add_0", "tapa_aie_Z_0"],
            "kernel names are sorted independently of instance IDs"
        );
        assert_eq!(
            report.per_kernel["tapa_aie_Add_0"],
            AieKernelReport {
                tile: AieTile { column: 23, row: 0 },
                program_bytes: 0,
                data_bytes: None,
            },
            "an explicit zero program size is preserved"
        );
        assert_eq!(
            report.per_kernel["tapa_aie_Z_0"].tile,
            AieTile { column: 25, row: 1 }
        );
        assert_eq!(report.per_kernel["tapa_aie_Z_0"].program_bytes, 11668);
        assert!(report
            .per_kernel
            .values()
            .all(|kernel| kernel.data_bytes.is_none()));
    }

    #[test]
    fn missing_and_malformed_reports_have_typed_path_aware_errors() {
        let temp = tempfile::tempdir().expect("temporary graph directory");
        let error = read_report(graph_dir(&temp)).expect_err("missing compiler report");
        match error {
            XilinxError::AieReportIo { path, .. } => {
                assert!(
                    path.ends_with("Work/reports/compiler_report.json"),
                    "{path}"
                );
            }
            other => panic!("unexpected error: {other}"),
        }

        let temp = write_reports(COMPILER, Some("Note: All sizes are in bits\n"));
        let error = read_report(graph_dir(&temp)).expect_err("wrong program-memory units");
        match error {
            XilinxError::AieReportParse { path, detail } => {
                assert!(path.ends_with("Work/reports/report_pm.txt"), "{path}");
                assert!(detail.contains("unit declaration"), "{detail}");
            }
            other => panic!("unexpected error: {other}"),
        }

        let temp = write_reports("{not JSON", Some(PM));
        let error = read_report(graph_dir(&temp)).expect_err("malformed JSON");
        assert!(matches!(
            error,
            XilinxError::AieReportJson { path, .. }
                if path.ends_with("Work/reports/compiler_report.json")
        ));
    }
}
