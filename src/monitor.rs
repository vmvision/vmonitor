use serde::Serialize;
use sysinfo::{Networks, RefreshKind, System};

#[derive(Serialize, Debug)]
pub struct SystemInfo {
    cpu_usage: f32,
    total_memory: u64,
    used_memory: u64,
}

#[derive(Serialize, Debug)]
pub struct NetworkInfo {
    download: u64,
    upload: u64,
}

pub fn collect_system_info(system: &mut System) -> SystemInfo {
    system.refresh_specifics(RefreshKind::everything());

    SystemInfo {
        cpu_usage: system.global_cpu_usage(),
        total_memory: system.total_memory(),
        used_memory: system.used_memory(),
    }
}

pub fn collect_network_info(networks: &mut Networks) -> NetworkInfo {
    networks.refresh(true);

    let mut download = 0;
    let mut upload = 0;

    for network in networks.list().values() {
        download += network.total_received();
        upload += network.total_transmitted();
    }

    NetworkInfo { download, upload }
}
