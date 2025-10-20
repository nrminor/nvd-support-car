use std::{env, fs::File, io::Write, num::NonZeroUsize, thread};

use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub fn setup_tracing() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}

pub fn init_error_formatter() -> color_eyre::Result<()> {
    color_eyre::install()
}

pub fn checks() {
    rayon::scope(|s| {
        s.spawn(|_| check_cpu_cores());
        s.spawn(|_| check_storage_type());
        s.spawn(|_| check_temp_directory());
    });
}

fn check_cpu_cores() {
    let cores = thread::available_parallelism()
        .map(NonZeroUsize::get)
        .unwrap_or(1);

    info!("Detected {cores} CPU cores");

    if cores < 2 {
        warn!(
            "Only {cores} CPU core detected. This tool benefits significantly from multiple cores. \
            Consider running on a machine with more cores for better performance."
        );
    } else if cores < 4 {
        warn!(
            "Only {cores} CPU cores detected. This tool performs best with 4 or more cores. \
            Performance may be limited."
        );
    }
}

fn check_storage_type() {
    let is_likely_ssd = check_if_ssd();

    if is_likely_ssd {
        info!("Storage appears to be SSD (optimal for performance)");
    } else {
        warn!(
            "Storage may not be an SSD. This tool performs significantly better on SSDs \
            due to intensive I/O operations. Consider using SSD storage for optimal performance."
        );
    }
}

fn check_if_ssd() -> bool {
    #[cfg(target_os = "linux")]
    {
        check_linux_storage_type()
    }

    #[cfg(target_os = "macos")]
    {
        check_macos_storage_type()
    }

    #[cfg(target_os = "windows")]
    {
        check_windows_storage_type()
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        true
    }
}

#[cfg(target_os = "linux")]
fn check_linux_storage_type() -> bool {
    for device in ["sda", "nvme0n1", "vda", "xvda"] {
        let rotational_path = format!("/sys/block/{device}/queue/rotational");
        if let Ok(content) = std::fs::read_to_string(&rotational_path)
            && content.trim() == "0"
        {
            return true;
        }
    }

    false
}

#[cfg(target_os = "macos")]
fn check_macos_storage_type() -> bool {
    let output = std::process::Command::new("diskutil")
        .args(["info", "/"])
        .output()
        .ok();

    if let Some(output) = output {
        let output_str = String::from_utf8_lossy(&output.stdout);
        let normalized = output_str.replace('\t', " ");
        let lines: Vec<&str> = normalized.lines().collect();

        for line in lines {
            if line.contains("Solid State") && line.contains("Yes") {
                return true;
            }
            if line.contains("Media Type") && line.contains("SSD") {
                return true;
            }
            if line.contains("Protocol")
                && (line.contains("PCI-Express") || line.contains("NVMe"))
            {
                return true;
            }
        }
    }

    true
}

#[cfg(target_os = "windows")]
fn check_windows_storage_type() -> bool {
    let output = std::process::Command::new("powershell")
        .args(&[
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "(Get-PhysicalDisk | Where MediaType -eq 'SSD').Count -gt 0",
        ])
        .output()
        .ok();

    if let Some(output) = output {
        let output_str = String::from_utf8_lossy(&output.stdout);
        matches!(output_str.trim(), "True" | "true")
    } else {
        false
    }
}

fn check_temp_directory() {
    let temp_dir = env::temp_dir();
    let test_file = temp_dir.join("checkle_preflight_test");

    let can_write = File::create(&test_file)
        .and_then(|mut f| f.write_all(b"test"))
        .and_then(|()| std::fs::remove_file(&test_file))
        .is_ok();

    if !can_write {
        warn!(
            "Cannot write to temporary directory. Some operations may fail. \
            Please ensure {} is writable.",
            temp_dir.display()
        );
    }
}
