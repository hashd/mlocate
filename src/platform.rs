#[cfg(target_os = "macos")]
pub mod path {
    pub fn default_db_dir() -> String {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        format!("{}/Library/Caches/mlocate", home)
    }

    pub fn default_localpaths() -> Vec<String> {
        vec!["/Users".to_string()]
    }

    pub fn default_prunepaths() -> Vec<String> {
        vec![
            "/Volumes".to_string(),
            "/dev".to_string(),
        ]
    }
}

#[cfg(target_os = "linux")]
pub mod path {
    pub fn default_db_dir() -> String {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        format!("{}/.cache/mlocate", home)
    }

    pub fn default_localpaths() -> Vec<String> {
        vec![
            "/home".to_string(),
            "/etc".to_string(),
            "/usr".to_string(),
            "/opt".to_string(),
        ]
    }

    pub fn default_prunepaths() -> Vec<String> {
        vec![
            "/proc".to_string(),
            "/sys".to_string(),
            "/dev".to_string(),
            "/run".to_string(),
            "/tmp".to_string(),
            "/var/tmp".to_string(),
            "/snap".to_string(),
            "/lost+found".to_string(),
        ]
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub mod path {
    pub fn default_db_dir() -> String {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        format!("{}/.cache/mlocate", home)
    }

    pub fn default_localpaths() -> Vec<String> {
        vec!["/".to_string()]
    }

    pub fn default_prunepaths() -> Vec<String> {
        vec![]
    }
}

pub fn db_path(override_path: Option<&str>) -> String {
    if let Some(p) = override_path {
        return p.to_string();
    }
    format!("{}/mlocate.db", path::default_db_dir())
}

pub fn tmp_db_path(override_path: Option<&str>) -> String {
    let base = if let Some(p) = override_path {
        p.to_string()
    } else {
        format!("{}/mlocate.db", path::default_db_dir())
    };
    format!("{}.tmp", base)
}

pub fn ensure_cache_dir(db_path_str: &str) -> Result<(), std::io::Error> {
    if let Some(parent) = std::path::Path::new(db_path_str).parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

pub fn set_niceness() {
    #[cfg(target_os = "macos")]
    {
        use mach2::mach_port::mach_port_deallocate;
        use mach2::thread_policy::{thread_background_policy, thread_policy_set, THREAD_BACKGROUND_POLICY, THREAD_BACKGROUND_POLICY_COUNT};
        use mach2::traps::mach_task_self;

        let mut policy = thread_background_policy { priority: 0 };
        unsafe {
            let thread = mach2::mach_init::mach_thread_self();
            thread_policy_set(
                thread,
                THREAD_BACKGROUND_POLICY,
                &mut policy as *mut _ as *mut std::ffi::c_int,
                THREAD_BACKGROUND_POLICY_COUNT,
            );
            mach_port_deallocate(mach_task_self(), thread);
        }
    }

    #[cfg(target_os = "linux")]
    {
        unsafe {
            let ret = libc::setpriority(libc::PRIO_PROCESS, 0, 10);
            if ret != 0 {
                eprintln!("Warning: Failed to set process priority (errno: {})", std::io::Error::last_os_error().raw_os_error().unwrap_or(-1));
            }
        }
    }
}

pub fn default_parallel() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .min(4)
}

pub fn cleanup_stale_tmp(db_dir: &str) -> Result<(), std::io::Error> {
    let tmp_db = format!("{}/mlocate.db.tmp", db_dir);
    if std::path::Path::new(&tmp_db).exists() {
        std::fs::remove_file(&tmp_db)?;
    }
    Ok(())
}
