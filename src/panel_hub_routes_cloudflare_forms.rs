//! Cloudflare DNS route query/form types and list-option helpers.

use crate::panel_hub_pages_cloudflare_pager::{
    CfTableOpts, dns_mode_from_query, dns_order_from_query, dns_page_from_query,
    dns_per_page_from_query, dns_search_from_query, dns_sort_from_query, manage_list_url,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CfQuery {
    pub tab: Option<String>,
    pub domain: Option<String>,
    #[serde(rename = "type")]
    pub filter_type: Option<String>,
    pub q: Option<String>,
    pub page: Option<String>,
    pub per_page: Option<String>,
    pub mode: Option<String>,
    pub sort: Option<String>,
    pub order: Option<String>,
    pub notice: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CfDomainForm {
    pub domain: String,
    #[serde(default)]
    pub filter_type: Option<String>,
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub page: Option<String>,
    #[serde(default)]
    pub per_page: Option<String>,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub sort: Option<String>,
    #[serde(default)]
    pub order: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CfAddForm {
    pub domain: String,
    pub record_type: String,
    pub name: String,
    pub content: String,
    pub ttl: Option<u32>,
    #[serde(default)]
    pub priority: Option<String>,
    pub proxied: Option<String>,
    #[serde(default)]
    pub filter_type: Option<String>,
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub page: Option<String>,
    #[serde(default)]
    pub per_page: Option<String>,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub sort: Option<String>,
    #[serde(default)]
    pub order: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CfRecordForm {
    pub domain: String,
    pub record_id: String,
    #[serde(default)]
    pub filter_type: Option<String>,
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub page: Option<String>,
    #[serde(default)]
    pub per_page: Option<String>,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub sort: Option<String>,
    #[serde(default)]
    pub order: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CfUpdateForm {
    pub domain: String,
    pub record_id: String,
    pub name: String,
    pub content: String,
    pub ttl: Option<u32>,
    #[serde(default)]
    pub priority: Option<String>,
    pub proxied: Option<String>,
    #[serde(default)]
    pub filter_type: Option<String>,
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub page: Option<String>,
    #[serde(default)]
    pub per_page: Option<String>,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub sort: Option<String>,
    #[serde(default)]
    pub order: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CfProxyForm {
    pub domain: String,
    pub record_id: String,
    pub proxied: String,
    #[serde(default)]
    pub filter_type: Option<String>,
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub page: Option<String>,
    #[serde(default)]
    pub per_page: Option<String>,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub sort: Option<String>,
    #[serde(default)]
    pub order: Option<String>,
}

pub fn table_opts_from_query(query: &CfQuery) -> CfTableOpts {
    let per_raw = query.per_page.as_deref().unwrap_or("10");
    let mode = if per_raw.eq_ignore_ascii_case("all") {
        "scroll".to_string()
    } else {
        dns_mode_from_query(query.mode.as_deref().unwrap_or("page")).to_string()
    };
    CfTableOpts {
        filter_type: query.filter_type.clone().unwrap_or_default(),
        q: dns_search_from_query(query.q.as_deref().unwrap_or("")),
        page: dns_page_from_query(query.page.as_deref().unwrap_or("1")),
        per_page: dns_per_page_from_query(per_raw),
        mode,
        sort: dns_sort_from_query(query.sort.as_deref().unwrap_or("name")).to_string(),
        order: dns_order_from_query(query.order.as_deref().unwrap_or("asc")).to_string(),
    }
}

pub fn manage_back(domain: &str, opts: &CfTableOpts) -> String {
    manage_list_url(domain, opts)
}

pub fn opts_from_form(
    filter_type: Option<&str>,
    q: Option<&str>,
    page: Option<&str>,
    per_page: Option<&str>,
    mode: Option<&str>,
    sort: Option<&str>,
    order: Option<&str>,
) -> CfTableOpts {
    CfTableOpts {
        filter_type: filter_type.unwrap_or("").to_string(),
        q: dns_search_from_query(q.unwrap_or("")),
        page: dns_page_from_query(page.unwrap_or("1")),
        per_page: dns_per_page_from_query(per_page.unwrap_or("10")),
        mode: dns_mode_from_query(mode.unwrap_or("page")).to_string(),
        sort: dns_sort_from_query(sort.unwrap_or("name")).to_string(),
        order: dns_order_from_query(order.unwrap_or("asc")).to_string(),
    }
}

pub fn parse_optional_u16(raw: Option<&str>) -> Option<u16> {
    raw.map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<u16>().ok())
}
