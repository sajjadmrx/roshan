//! Developer harness: prints what discovery finds on this machine and how long
//! it takes. `cargo run -p roshan-platform --example list_apps [filter]`
use std::time::Instant;

fn main() {
    let filter = std::env::args().nth(1).map(|f| f.to_lowercase());
    let started = Instant::now();
    let apps = roshan_platform::discover_apps();
    let discovery = started.elapsed();

    let started = Instant::now();
    let mut with_icon = 0;
    for app in &apps {
        if filter
            .as_ref()
            .is_some_and(|f| !app.name.to_lowercase().contains(f))
        {
            continue;
        }
        let icon = roshan_platform::app_icon(&app.target);
        with_icon += usize::from(icon.is_some());
        println!(
            "{:<40} icon={:<5} {}",
            app.name,
            icon.is_some(),
            app.detail.as_deref().unwrap_or("")
        );
        if filter.is_some() {
            println!("    {:?}", app.target);
        }
    }
    println!(
        "\n{} apps discovered in {:?}; {} icons in {:?}",
        apps.len(),
        discovery,
        with_icon,
        started.elapsed()
    );
}
