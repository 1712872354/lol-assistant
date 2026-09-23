//! Windows 进程命令行读取：`NtQueryInformationProcess(ProcessCommandLineInformation)`。
//! sysinfo/CIM 在国服场景常读空，本模块直连 NT API（已实测可读 LeagueClientUx）。

#![cfg(windows)]

use std::ffi::c_void;

#[repr(C)]
struct UnicodeString {
    length: u16,
    maximum_length: u16,
    buffer: *const u16,
}

extern "system" {
    fn NtQueryInformationProcess(
        process_handle: *mut c_void,
        process_information_class: u32,
        process_information: *mut c_void,
        process_information_length: u32,
        return_length: *mut u32,
    ) -> i32;

    fn OpenProcess(access: u32, inherit_handle: i32, process_id: u32) -> *mut c_void;
    fn CloseHandle(object: *mut c_void) -> i32;
}

const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
const PROCESS_QUERY_INFORMATION: u32 = 0x0400;
const PROCESS_VM_READ: u32 = 0x0010;
/// `ProcessCommandLineInformation`（Windows 8.1+）
const PROCESSINFOCLASS_CMDLINE: u32 = 60;

/// 读取进程完整命令行；失败返回 None。
pub fn read_command_line(pid: u32) -> Option<String> {
    if pid == 0 {
        return None;
    }
    unsafe {
        let mut handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            handle = OpenProcess(
                PROCESS_QUERY_INFORMATION | PROCESS_VM_READ | PROCESS_QUERY_LIMITED_INFORMATION,
                0,
                pid,
            );
        }
        if handle.is_null() {
            return None;
        }
        let out = query_command_line(handle);
        CloseHandle(handle);
        out
    }
}

unsafe fn query_command_line(handle: *mut c_void) -> Option<String> {
    let mut return_length: u32 = 0;
    let _ = NtQueryInformationProcess(
        handle,
        PROCESSINFOCLASS_CMDLINE,
        std::ptr::null_mut(),
        0,
        &mut return_length,
    );
    if return_length == 0 || return_length > 1024 * 1024 {
        return_length = 8192;
    }
    let mut buf = vec![0u8; return_length as usize];
    let status = NtQueryInformationProcess(
        handle,
        PROCESSINFOCLASS_CMDLINE,
        buf.as_mut_ptr().cast(),
        return_length,
        &mut return_length,
    );
    if status != 0 {
        return None;
    }
    if buf.len() < std::mem::size_of::<UnicodeString>() {
        return None;
    }
    let us = &*(buf.as_ptr() as *const UnicodeString);
    if us.buffer.is_null() || us.length == 0 {
        return None;
    }
    let len = (us.length as usize) / 2;
    // buffer 指向本次返回缓冲区内数据；在 buf 释放前拷出
    let slice = std::slice::from_raw_parts(us.buffer, len);
    let s = String::from_utf16_lossy(slice);
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_self_command_line() {
        let pid = std::process::id();
        let cmd = read_command_line(pid).expect("should read own cmdline");
        assert!(!cmd.is_empty());
    }
}
