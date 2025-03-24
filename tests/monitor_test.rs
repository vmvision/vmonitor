use sysinfo::System;

#[test]
fn test_system_info_collection() {
    let mut system = System::new();
    system.refresh_all();

    let system_info = vmonitor::monitor::collect_system_info(&mut system);

    // Basic sanity checks
    assert!(system_info.cpu_usage >= 0.0);
    assert!(system_info.memory_used <= system_info.memory_total);
    assert!(system_info.swap_used <= system_info.swap_total);
    assert!(system_info.process_count > 0);

    // Load average checks
    assert!(system_info.load_avg.one >= 0.0);
    assert!(system_info.load_avg.five >= 0.0);
    assert!(system_info.load_avg.fifteen >= 0.0);
}
