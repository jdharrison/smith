//! Execution-independent launch policy and pinned llama.cpp b11429 offload evidence parsing.

const CPU_ARGS: &[&str] = &["--device", "none", "--gpu-layers", "0", "--fit", "off"];
const VULKAN_ARGS: &[&str] = &[
    "--device",
    "Vulkan0",
    "--split-mode",
    "none",
    "--gpu-layers",
    "all",
    "--fit",
    "off",
];
const EVIDENCE_PREFIX: &str = "load_tensors: offloaded ";
const EVIDENCE_SUFFIX: &str = " layers to GPU";
const MAX_EVIDENCE_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    Cpu,
    Vulkan,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActualMode {
    Cpu,
    Gpu,
    GpuAndCpu,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvidenceError {
    TooLarge { max_bytes: usize },
    Missing,
    Duplicate,
    Malformed,
    InvalidLayerCount,
    ZeroOffload { total: u64 },
    OffloadedExceedsTotal { offloaded: u64, total: u64 },
}

pub fn launch_args(backend: Backend) -> &'static [&'static str] {
    match backend {
        Backend::Cpu => CPU_ARGS,
        Backend::Vulkan => VULKAN_ARGS,
    }
}

pub fn actual_mode(backend: Backend, stderr: &str) -> Result<ActualMode, EvidenceError> {
    if backend == Backend::Cpu {
        return Ok(ActualMode::Cpu);
    }
    if stderr.len() > MAX_EVIDENCE_BYTES {
        return Err(EvidenceError::TooLarge {
            max_bytes: MAX_EVIDENCE_BYTES,
        });
    }

    let mut evidence = None;
    for line in stderr.lines() {
        if line.starts_with(EVIDENCE_PREFIX) && evidence.replace(line).is_some() {
            return Err(EvidenceError::Duplicate);
        }
    }

    parse_vulkan_evidence(evidence.ok_or(EvidenceError::Missing)?)
}

fn parse_vulkan_evidence(line: &str) -> Result<ActualMode, EvidenceError> {
    let counts = line
        .strip_prefix(EVIDENCE_PREFIX)
        .and_then(|value| value.strip_suffix(EVIDENCE_SUFFIX))
        .ok_or(EvidenceError::Malformed)?;
    let (offloaded, total) = counts.split_once('/').ok_or(EvidenceError::Malformed)?;
    let offloaded = parse_decimal(offloaded)?;
    let total = parse_decimal(total)?;

    if offloaded > total {
        return Err(EvidenceError::OffloadedExceedsTotal { offloaded, total });
    }
    if offloaded == 0 {
        return Err(EvidenceError::ZeroOffload { total });
    }
    if offloaded == total {
        Ok(ActualMode::Gpu)
    } else {
        Ok(ActualMode::GpuAndCpu)
    }
}

fn parse_decimal(value: &str) -> Result<u64, EvidenceError> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(EvidenceError::InvalidLayerCount);
    }
    value
        .parse::<u64>()
        .map_err(|_| EvidenceError::InvalidLayerCount)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_flags_are_exact() {
        assert_eq!(
            launch_args(Backend::Cpu),
            ["--device", "none", "--gpu-layers", "0", "--fit", "off"]
        );
        assert_eq!(
            launch_args(Backend::Vulkan),
            [
                "--device",
                "Vulkan0",
                "--split-mode",
                "none",
                "--gpu-layers",
                "all",
                "--fit",
                "off"
            ]
        );
    }

    #[test]
    fn full_offload_is_gpu() {
        assert_eq!(
            actual_mode(
                Backend::Vulkan,
                "load_tensors: offloaded 33/33 layers to GPU"
            ),
            Ok(ActualMode::Gpu)
        );
    }

    #[test]
    fn partial_offload_is_gpu_and_cpu() {
        assert_eq!(
            actual_mode(
                Backend::Vulkan,
                "load_tensors: offloaded 12/33 layers to GPU"
            ),
            Ok(ActualMode::GpuAndCpu)
        );
    }

    #[test]
    fn zero_offload_is_an_error() {
        assert_eq!(
            actual_mode(
                Backend::Vulkan,
                "load_tensors: offloaded 0/33 layers to GPU"
            ),
            Err(EvidenceError::ZeroOffload { total: 33 })
        );
    }

    #[test]
    fn missing_evidence_is_an_error() {
        assert_eq!(
            actual_mode(Backend::Vulkan, "server: listening"),
            Err(EvidenceError::Missing)
        );
    }

    #[test]
    fn malformed_evidence_is_an_error() {
        assert_eq!(
            actual_mode(
                Backend::Vulkan,
                "load_tensors: offloaded 33/33 layers to GPU extra"
            ),
            Err(EvidenceError::Malformed)
        );
        assert_eq!(
            actual_mode(
                Backend::Vulkan,
                "load_tensors: offloaded many/33 layers to GPU"
            ),
            Err(EvidenceError::InvalidLayerCount)
        );
    }

    #[test]
    fn duplicate_evidence_is_an_error() {
        let stderr = "load_tensors: offloaded 33/33 layers to GPU\n\
                      load_tensors: offloaded 33/33 layers to GPU";
        assert_eq!(
            actual_mode(Backend::Vulkan, stderr),
            Err(EvidenceError::Duplicate)
        );
    }

    #[test]
    fn offloaded_layers_cannot_exceed_total() {
        assert_eq!(
            actual_mode(
                Backend::Vulkan,
                "load_tensors: offloaded 34/33 layers to GPU"
            ),
            Err(EvidenceError::OffloadedExceedsTotal {
                offloaded: 34,
                total: 33
            })
        );
    }

    #[test]
    fn unrelated_lines_are_ignored() {
        let stderr = "ggml_vulkan: found device\nload_tensors: offloaded 33/33 layers to GPU\nserver: listening";
        assert_eq!(actual_mode(Backend::Vulkan, stderr), Ok(ActualMode::Gpu));
    }

    #[test]
    fn cpu_mode_is_independent_of_vulkan_evidence() {
        assert_eq!(
            actual_mode(Backend::Cpu, "malformed or missing Vulkan output"),
            Ok(ActualMode::Cpu)
        );
    }
}
