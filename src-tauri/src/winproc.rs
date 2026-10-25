//! Small Win32 process helpers.
//!
//! These replace shelling out to `taskkill`: spawn of `taskkill`/`cmd` from a
//! launcher is a classic heuristic antivirus engines flag, and it also fails on
//! systems where the shell is unavailable. Everything here talks to
//! `kernel32.dll` directly.

/// Terminates a process and every child process it spawned.
///
/// Returns `true` when at least the target process was terminated.
pub fn kill_tree(pid: u32) -> bool {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
            TH32CS_SNAPPROCESS,
        };
        use windows::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};

        unsafe fn children_of(parent: u32) -> Vec<u32> {
            let mut children = Vec::new();
            let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
                return children;
            };

            let mut entry = PROCESSENTRY32W {
                dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                ..Default::default()
            };

            if Process32FirstW(snapshot, &mut entry).is_ok() {
                loop {
                    if entry.th32ParentProcessID == parent {
                        children.push(entry.th32ProcessID);
                    }
                    if Process32NextW(snapshot, &mut entry).is_err() {
                        break;
                    }
                }
            }

            let _ = CloseHandle(snapshot);
            children
        }

        unsafe fn terminate(pid: u32) -> bool {
            let Ok(handle) = OpenProcess(PROCESS_TERMINATE, false, pid) else {
                return false;
            };
            let terminated = TerminateProcess(handle, 1).is_ok();
            let _ = CloseHandle(handle);
            terminated
        }

        unsafe fn kill_recursive(pid: u32) -> bool {
            let mut killed = false;
            for child in children_of(pid) {
                killed |= kill_recursive(child);
            }
            killed | terminate(pid)
        }

        unsafe { kill_recursive(pid) }
    }

    #[cfg(not(target_os = "windows"))]
    {
        std::process::Command::new("kill")
            .args(["-9", &pid.to_string()])
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }
}

/// One running process, from a name-only snapshot.
#[derive(Debug, Clone)]
pub struct ProcessEntry {
    pub pid: u32,
    pub parent_pid: u32,
    /// Executable file name only (no path, no memory information).
    pub name: String,
}

/// Lists running processes: PIDs and executable names, nothing else.
///
/// Deliberately avoids anything that reads process memory or performance
/// counters — a launcher polling this every few seconds looks like a scanner to
/// antivirus/EDR products otherwise.
pub fn list_processes() -> Vec<ProcessEntry> {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
            TH32CS_SNAPPROCESS,
        };

        let mut entries = Vec::new();

        unsafe {
            let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
                return entries;
            };

            let mut raw = PROCESSENTRY32W {
                dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                ..Default::default()
            };

            if Process32FirstW(snapshot, &mut raw).is_ok() {
                loop {
                    let name = String::from_utf16_lossy(
                        &raw.szExeFile[..raw
                            .szExeFile
                            .iter()
                            .position(|c| *c == 0)
                            .unwrap_or(raw.szExeFile.len())],
                    );
                    entries.push(ProcessEntry {
                        pid: raw.th32ProcessID,
                        parent_pid: raw.th32ParentProcessID,
                        name,
                    });
                    if Process32NextW(snapshot, &mut raw).is_err() {
                        break;
                    }
                }
            }

            let _ = CloseHandle(snapshot);
        }

        entries
    }

    #[cfg(not(target_os = "windows"))]
    {
        Vec::new()
    }
}

/// Full executable path of a process, or `None` when it cannot be opened.
pub fn exe_path(pid: u32) -> Option<std::path::PathBuf> {
    #[cfg(target_os = "windows")]
    {
        use windows::core::PWSTR;
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Threading::{
            OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
            PROCESS_QUERY_LIMITED_INFORMATION,
        };

        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;

            let mut buffer = [0u16; 1024];
            let mut size = buffer.len() as u32;
            let ok = QueryFullProcessImageNameW(
                handle,
                PROCESS_NAME_WIN32,
                PWSTR(buffer.as_mut_ptr()),
                &mut size,
            )
            .is_ok();

            let _ = CloseHandle(handle);

            if ok {
                Some(std::path::PathBuf::from(String::from_utf16_lossy(
                    &buffer[..size as usize],
                )))
            } else {
                None
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

/// True while the process is alive.
///
/// This is a wait on the process handle (zero timeout), not a scan: no process
/// enumeration, no memory or CPU counters.
pub fn is_running(pid: u32) -> bool {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Threading::{
            OpenProcess, WaitForSingleObject, PROCESS_ACCESS_RIGHTS,
        };

        // SYNCHRONIZE (0x00100000): wait-only access, the least privileged way
        // to ask "is this PID still alive?".
        const SYNCHRONIZE: u32 = 0x0010_0000;
        const WAIT_TIMEOUT: u32 = 0x0000_0102;

        unsafe {
            let Ok(handle) = OpenProcess(PROCESS_ACCESS_RIGHTS(SYNCHRONIZE), false, pid) else {
                return false;
            };
            let wait = WaitForSingleObject(handle, 0).0;
            let _ = CloseHandle(handle);
            wait == WAIT_TIMEOUT
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

/// Lower-cased names of the executables under `dir` (bounded walk).
fn exe_names_in(dir: &std::path::Path) -> std::collections::HashSet<String> {
    const MAX_DEPTH: usize = 8;
    const MAX_ENTRIES: usize = 100_000;

    let mut names = std::collections::HashSet::new();
    let mut stack = vec![(dir.to_path_buf(), 0usize)];
    let mut visited = 0usize;
    while let Some((current, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            visited += 1;
            if visited > MAX_ENTRIES {
                return names;
            }
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                if depth < MAX_DEPTH {
                    stack.push((entry.path(), depth + 1));
                }
            } else {
                let name = entry.file_name().to_string_lossy().to_lowercase();
                if name.ends_with(".exe") {
                    names.insert(name);
                }
            }
        }
    }
    names
}

/// Finds a running process whose executable lives inside `dir`.
///
/// Only processes whose image name matches an executable in `dir` are opened.
/// Opening a handle to every process on the machine each minute is the
/// enumeration pattern behaviour engines associate with injectors and stealers.
pub fn find_process_in_dir(dir: &std::path::Path) -> Option<u32> {
    if dir.as_os_str().is_empty() {
        return None;
    }

    let candidates = exe_names_in(dir);
    if candidates.is_empty() {
        return None;
    }

    for entry in list_processes() {
        if entry.pid == std::process::id() || !candidates.contains(&entry.name.to_lowercase()) {
            continue;
        }
        if let Some(path) = exe_path(entry.pid) {
            if path.starts_with(dir) {
                return Some(entry.pid);
            }
        }
    }
    None
}

/// Finds the process running exactly `exe`.
pub fn find_process_by_exe(exe: &std::path::Path) -> Option<u32> {
    let wanted = exe.file_name()?.to_string_lossy().to_lowercase();
    if wanted.is_empty() {
        return None;
    }

    for entry in list_processes() {
        if entry.name.to_lowercase() != wanted {
            continue;
        }
        if let Some(path) = exe_path(entry.pid) {
            if same_path(&path, exe) {
                return Some(entry.pid);
            }
        }
    }
    None
}

/// Case-insensitive path comparison (Windows semantics).
fn same_path(a: &std::path::Path, b: &std::path::Path) -> bool {
    a.to_string_lossy().eq_ignore_ascii_case(&b.to_string_lossy())
}


#[cfg(all(test, target_os = "windows"))]
mod tests {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x08000000;

    #[test]
    fn kills_a_running_process() {
        let mut child = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", "Start-Sleep -Seconds 30"])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .expect("spawn a throwaway process");

        let pid = child.id();
        assert!(super::kill_tree(pid), "process should be terminated");
        std::thread::sleep(std::time::Duration::from_millis(500));
        let _ = child.wait();
    }

    #[test]
    fn unknown_pid_reports_failure() {
        assert!(!super::kill_tree(999_999));
    }
}
