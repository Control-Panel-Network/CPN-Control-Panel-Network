//! Tile-hub overview HTML for sidebar categories (Websites, WordPress, FTP, SSL, …).
//!
//! Matches Users & Plans: section headers plus LIVE/SCAFFOLD cards for each child action.
//! List/manage UIs stay on dedicated routes (`/websites/list`, `/docker/list`, …).

use crate::panel_hub_defs::{
    docker_hub_sections, ftp_hub_sections, litespeed_hub_sections, php_hub_sections,
    plugins_hub_sections, ssl_hub_sections, websites_hub_sections, wordpress_hub_sections,
};
use crate::panel_hubs::category_hub_main;

pub fn websites_hub_main() -> String {
    category_hub_main(
        "Websites",
        "Main domains, sub-domains, and create actions for hosted sites.",
        websites_hub_sections(),
    )
}

pub fn wordpress_hub_main() -> String {
    category_hub_main(
        "WordPress",
        "Install and manage WordPress on main websites and sub-domains.",
        wordpress_hub_sections(),
    )
}

pub fn ftp_hub_main() -> String {
    category_hub_main(
        "FTP",
        "Jailed SFTP accounts for website and sub-domain document roots.",
        ftp_hub_sections(),
    )
}

pub fn ssl_hub_main() -> String {
    category_hub_main(
        "SSL",
        "Site certificates, panel hostname SSL, and mail server SSL.",
        ssl_hub_sections(),
    )
}

pub fn plugins_hub_main() -> String {
    category_hub_main(
        "Plugins",
        "Installed plugins, Store catalog, and Host packages for this server.",
        plugins_hub_sections(),
    )
}

pub fn docker_hub_main() -> String {
    category_hub_main(
        "Docker",
        "Active containers, images, compose stacks, and create on this host.",
        docker_hub_sections(),
    )
}

pub fn php_hub_main() -> String {
    category_hub_main(
        "PHP",
        "Extensions, php.ini configurations, host default, and tuning.",
        php_hub_sections(),
    )
}

pub fn litespeed_hub_main() -> String {
    category_hub_main(
        "LiteSpeed",
        "Open LiteSpeed WebAdmin and plan or serial management when installed.",
        litespeed_hub_sections(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn websites_hub_links_list_not_overview() {
        let html = websites_hub_main();
        assert!(html.contains("/websites/list"));
        assert!(html.contains("/websites/create"));
        assert!(html.contains("/subdomains"));
        assert!(html.contains("List Websites"));
        assert!(html.contains("hub-badge live") || html.contains(">Live</span>"));
    }

    #[test]
    fn docker_hub_links_list_route() {
        // Section defs always include list/create/images. Rendered HTML is
        // feature-gated and empty when Docker/Podman is not installed on the host.
        let hrefs: Vec<&str> = crate::panel_hub_defs::docker_hub_sections()
            .into_iter()
            .flat_map(|(_, tiles)| tiles.into_iter().map(|t| t.href))
            .collect();
        assert!(hrefs.contains(&"/docker/list"));
        assert!(hrefs.contains(&"/docker/create"));
        assert!(hrefs.contains(&"/docker/images"));
        if crate::panel_feature_gate::docker_installed() {
            let html = docker_hub_main();
            assert!(html.contains("/docker/list"));
            assert!(html.contains("/docker/create"));
            assert!(html.contains("/docker/images"));
        }
    }

    #[test]
    fn php_hub_links_children() {
        let html = php_hub_main();
        assert!(html.contains("/server/php/extensions"));
        assert!(html.contains("/server/php/configs"));
        assert!(html.contains("/server/php/tuning"));
    }
}
