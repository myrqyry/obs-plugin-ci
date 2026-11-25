// Synthetic export fixture only. Not a real OBS plugin or SDK/ABI test.
#[unsafe(no_mangle)]
pub extern "C" fn obs_module_load() -> bool {
    true
}

#[unsafe(no_mangle)]
pub extern "C" fn obs_module_set_pointer(_: *mut u8) {}

#[unsafe(no_mangle)]
pub extern "C" fn obs_module_ver() -> u32 {
    1
}
