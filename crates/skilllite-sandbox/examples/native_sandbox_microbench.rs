#![cfg(unix)]

use skilllite_sandbox::runner::{ResourceLimits, RuntimePaths, SandboxConfig};
use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy)]
enum BenchLevel {
    NativeSpawn,
    Sandbox,
    Docker,
    Srt,
}

#[derive(Debug)]
struct BenchConfig {
    level: BenchLevel,
    iterations: usize,
    warmup: usize,
    program: PathBuf,
    docker_image: String,
    srt_bin: String,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let config = parse_args()?;
    let skill_dir = tempfile::tempdir().map_err(|e| format!("create temp skill dir: {e}"))?;
    let runtime = RuntimePaths {
        // The sandbox backend only needs a resolved interpreter path. Pointing the
        // python slot at /usr/bin/true avoids starting Python while reusing the
        // existing platform sandbox path without changing production runtime semantics.
        python: config.program.clone(),
        node: config.program.clone(),
        node_modules: None,
        env_dir: PathBuf::new(),
    };
    let sandbox_config = SandboxConfig {
        name: "native-sandbox-microbench".to_string(),
        entry_point: String::new(),
        language: "python".to_string(),
        network_enabled: false,
        network_outbound: Vec::new(),
        uses_playwright: false,
    };
    let limits = ResourceLimits {
        max_memory_mb: 256,
        timeout_secs: 5,
    };

    for _ in 0..config.warmup {
        run_once(&config, skill_dir.path(), &runtime, &sandbox_config, limits)?;
    }

    let mut samples = Vec::with_capacity(config.iterations);
    for _ in 0..config.iterations {
        samples.push(run_once(
            &config,
            skill_dir.path(),
            &runtime,
            &sandbox_config,
            limits,
        )?);
    }

    samples.sort_unstable();
    print_report(&config, &samples);
    Ok(())
}

fn parse_args() -> Result<BenchConfig, String> {
    let mut level = None;
    let mut iterations = 200usize;
    let mut warmup = 20usize;
    let mut program = PathBuf::from("/usr/bin/true");
    let mut docker_image = "alpine:3.20".to_string();
    let mut srt_bin = "srt".to_string();

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--level" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--level requires native or sandbox".to_string())?;
                level = Some(match value.as_str() {
                    "native" | "l1" => BenchLevel::NativeSpawn,
                    "sandbox" | "l2" => BenchLevel::Sandbox,
                    "docker" => BenchLevel::Docker,
                    "srt" => BenchLevel::Srt,
                    _ => {
                        return Err(
                            "--level must be native/l1, sandbox/l2, docker, or srt".to_string()
                        )
                    }
                });
            }
            "--iterations" | "-n" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--iterations requires a number".to_string())?;
                iterations = value
                    .parse()
                    .map_err(|_| "--iterations must be a positive integer".to_string())?;
            }
            "--warmup" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--warmup requires a number".to_string())?;
                warmup = value
                    .parse()
                    .map_err(|_| "--warmup must be a non-negative integer".to_string())?;
            }
            "--program" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--program requires an absolute path".to_string())?;
                program = PathBuf::from(value);
            }
            "--docker-image" => {
                docker_image = args
                    .next()
                    .ok_or_else(|| "--docker-image requires an image name".to_string())?;
            }
            "--srt-bin" => {
                srt_bin = args
                    .next()
                    .ok_or_else(|| "--srt-bin requires a command path or name".to_string())?;
            }
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            _ => return Err(format!("unknown argument: {arg}")),
        }
    }

    if iterations == 0 {
        return Err("--iterations must be greater than zero".to_string());
    }
    if !program.is_absolute() {
        return Err("--program must be an absolute path".to_string());
    }
    if !program.exists() {
        return Err(format!("program not found: {}", program.display()));
    }

    Ok(BenchConfig {
        level: level.unwrap_or(BenchLevel::Sandbox),
        iterations,
        warmup,
        program,
        docker_image,
        srt_bin,
    })
}

fn run_once(
    config: &BenchConfig,
    skill_dir: &Path,
    runtime: &RuntimePaths,
    sandbox_config: &SandboxConfig,
    limits: ResourceLimits,
) -> Result<Duration, String> {
    let start = Instant::now();
    match config.level {
        BenchLevel::NativeSpawn => {
            let status = Command::new(&config.program)
                .status()
                .map_err(|e| format!("spawn native program: {e}"))?;
            if !status.success() {
                return Err(format!("native program exited with status: {status}"));
            }
        }
        BenchLevel::Sandbox => {
            let result = execute_sandboxed(skill_dir, runtime, sandbox_config, limits)?;
            if result.exit_code != 0 {
                return Err(format!(
                    "sandboxed program exited with code {}: {}",
                    result.exit_code, result.stderr
                ));
            }
        }
        BenchLevel::Docker => {
            let status = Command::new("docker")
                .args([
                    "run",
                    "--rm",
                    "--network=none",
                    "--security-opt=no-new-privileges",
                    &config.docker_image,
                    "/bin/true",
                ])
                .status()
                .map_err(|e| format!("spawn docker run: {e}"))?;
            if !status.success() {
                return Err(format!("docker run exited with status: {status}"));
            }
        }
        BenchLevel::Srt => {
            let status = Command::new(&config.srt_bin)
                .arg(&config.program)
                .status()
                .map_err(|e| format!("spawn SRT command: {e}"))?;
            if !status.success() {
                return Err(format!("SRT command exited with status: {status}"));
            }
        }
    }
    Ok(start.elapsed())
}

#[cfg(target_os = "macos")]
fn execute_sandboxed(
    skill_dir: &Path,
    runtime: &RuntimePaths,
    config: &SandboxConfig,
    limits: ResourceLimits,
) -> Result<skilllite_sandbox::runner::ExecutionResult, String> {
    skilllite_sandbox::macos::execute_with_limits(skill_dir, runtime, config, "{}", limits)
        .map_err(|e| format!("macOS sandbox execution failed: {e}"))
}

#[cfg(target_os = "linux")]
fn execute_sandboxed(
    skill_dir: &Path,
    runtime: &RuntimePaths,
    config: &SandboxConfig,
    limits: ResourceLimits,
) -> Result<skilllite_sandbox::runner::ExecutionResult, String> {
    skilllite_sandbox::linux::execute_with_limits(skill_dir, runtime, config, "{}", limits)
        .map_err(|e| format!("Linux sandbox execution failed: {e}"))
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn execute_sandboxed(
    _skill_dir: &Path,
    _runtime: &RuntimePaths,
    _config: &SandboxConfig,
    _limits: ResourceLimits,
) -> Result<skilllite_sandbox::runner::ExecutionResult, String> {
    Err("native sandbox microbenchmark currently supports macOS and Linux".to_string())
}

fn print_report(config: &BenchConfig, samples: &[Duration]) {
    let total_ns: u128 = samples.iter().map(Duration::as_nanos).sum();
    let avg_ms = total_ns as f64 / samples.len() as f64 / 1_000_000.0;
    let min_ms = duration_ms(samples[0]);
    let p50_ms = duration_ms(percentile(samples, 50));
    let p95_ms = duration_ms(percentile(samples, 95));
    let p99_ms = duration_ms(percentile(samples, 99));
    let max_ms = duration_ms(samples[samples.len() - 1]);

    println!("native sandbox microbench");
    println!("level: {:?}", config.level);
    println!("program: {}", config.program.display());
    if matches!(config.level, BenchLevel::Docker) {
        println!("docker_image: {}", config.docker_image);
    }
    if matches!(config.level, BenchLevel::Srt) {
        println!("srt_bin: {}", config.srt_bin);
    }
    println!("iterations: {}", config.iterations);
    println!("warmup: {}", config.warmup);
    println!("avg_ms: {avg_ms:.3}");
    println!("min_ms: {min_ms:.3}");
    println!("p50_ms: {p50_ms:.3}");
    println!("p95_ms: {p95_ms:.3}");
    println!("p99_ms: {p99_ms:.3}");
    println!("max_ms: {max_ms:.3}");
    match config.level {
        BenchLevel::Docker => {
            if let Some(rss_mb) = docker_container_peak_rss_mb(&config.docker_image) {
                println!("container_peak_rss_mb: {rss_mb:.3}");
            }
        }
        BenchLevel::NativeSpawn | BenchLevel::Sandbox | BenchLevel::Srt => {
            if let Some(rss_mb) = child_peak_rss_mb() {
                println!("child_peak_rss_mb: {rss_mb:.3}");
            }
        }
    }
}

fn docker_container_peak_rss_mb(image: &str) -> Option<f64> {
    let name = format!("skilllite-core-mem-{}", std::process::id());
    let run_status = Command::new("docker")
        .args([
            "run",
            "-d",
            "--name",
            &name,
            "--network=none",
            "--security-opt=no-new-privileges",
            image,
            "sh",
            "-c",
            "sleep 2",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .ok()?;
    if !run_status.success() {
        return None;
    }

    let stats = Command::new("docker")
        .args(["stats", "--no-stream", "--format", "{{.MemUsage}}", &name])
        .output()
        .ok();
    let _ = Command::new("docker")
        .args(["rm", "-f", &name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    let output = stats?;
    if !output.status.success() {
        return None;
    }
    parse_docker_mem_usage(std::str::from_utf8(&output.stdout).ok()?.trim())
}

fn parse_docker_mem_usage(mem_usage: &str) -> Option<f64> {
    let first = mem_usage.split('/').next()?.trim();
    let value = first
        .trim_end_matches("B")
        .trim_end_matches("i")
        .trim_end_matches("K")
        .trim_end_matches("M")
        .trim_end_matches("G")
        .parse::<f64>()
        .ok()?;

    if first.contains("GiB") || first.contains("GB") {
        Some(value * 1024.0)
    } else if first.contains("KiB") || first.contains("KB") {
        Some(value / 1024.0)
    } else if first.contains("B") && !first.contains("MB") && !first.contains("MiB") {
        Some(value / 1024.0 / 1024.0)
    } else {
        Some(value)
    }
}

fn percentile(samples: &[Duration], percentile: usize) -> Duration {
    let idx = samples.len().saturating_mul(percentile).saturating_sub(1) / 100;
    samples[idx.min(samples.len() - 1)]
}

fn duration_ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn child_peak_rss_mb() -> Option<f64> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    let rc = unsafe { libc::getrusage(libc::RUSAGE_CHILDREN, usage.as_mut_ptr()) };
    if rc != 0 {
        return None;
    }
    let usage = unsafe { usage.assume_init() };
    #[cfg(target_os = "macos")]
    {
        Some(usage.ru_maxrss as f64 / 1024.0 / 1024.0)
    }
    #[cfg(target_os = "linux")]
    {
        Some(usage.ru_maxrss as f64 / 1024.0)
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn child_peak_rss_mb() -> Option<f64> {
    None
}

fn print_usage() {
    println!(
        "Usage: cargo run -p skilllite-sandbox --example native_sandbox_microbench -- \\
         --level <native|sandbox|docker|srt> [--iterations N] [--warmup N] \\
         [--program /usr/bin/true] [--docker-image alpine:3.20] [--srt-bin srt]"
    );
}
