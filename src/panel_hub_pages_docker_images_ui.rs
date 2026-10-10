//! Docker Images page UI (Hub search, local image table, prune).

use crate::panel_hub_pages_docker::{html_escape, toolbar, urlencoding_simple};
use crate::panel_hub_pages_docker_list::{DockerListQuery, docker_list_toolbar, paginate_slice};
use crate::panel_hubs::feature_shell;
use crate::panel_ops_docker::{DockerImageRow, docker_status, list_images_detailed};
use crate::panel_ops_docker_images::{DockerHubSearchHit, image_in_use};
use crate::panel_ops_docker_inventory::filter_images;

fn search_results_html(hits: &[DockerHubSearchHit]) -> String {
    if hits.is_empty() {
        return String::new();
    }
    let mut body = String::from(
        r#"<div class="table-wrap" style="margin-top:12px;"><table class="data-table">
      <thead><tr>
        <th scope="col">Repository</th>
        <th scope="col">Description</th>
        <th scope="col">Stars</th>
        <th scope="col">Actions</th>
      </tr></thead><tbody>"#,
    );
    for hit in hits {
        let official = if hit.is_official {
            r#" <span class="plugin-badge">Official</span>"#
        } else {
            ""
        };
        let desc: String = hit.description.chars().take(120).collect();
        body.push_str(&format!(
            r#"<tr>
          <td><code>{name}</code>{official}</td>
          <td>{desc}</td>
          <td>{stars}</td>
          <td class="docker-actions">
            <form method="post" action="/docker/images/pull" class="inline-form" style="display:inline;">
              <input type="hidden" name="image" value="{name}:latest">
              <button type="submit" class="btn-primary">Pull</button>
            </form>
            <a class="btn-secondary" href="/docker/create?image={enc}">Create</a>
            <a class="btn-secondary" href="/docker/stacks?image={enc}&amp;template=custom">Stack</a>
          </td>
        </tr>"#,
            name = html_escape(&hit.name),
            official = official,
            desc = html_escape(&desc),
            stars = hit.star_count,
            enc = urlencoding_simple(&format!("{}:latest", hit.name)),
        ));
    }
    body.push_str("</tbody></table></div>");
    body
}

fn images_table(rows: &[&DockerImageRow], empty_message: &str) -> String {
    if rows.is_empty() {
        return format!(
            r#"<p class="empty-state">{}</p>"#,
            html_escape(empty_message)
        );
    }
    let mut body = String::from(
        r#"<div class="table-wrap docker-list-wrap"><table class="data-table docker-images-table docker-card-list">
      <thead><tr>
        <th scope="col">Image name</th>
        <th scope="col">Tags</th>
        <th scope="col">Size</th>
        <th scope="col">Actions</th>
      </tr></thead><tbody>"#,
    );
    for row in rows {
        let ref_name = if row.repository == "<none>" || row.tag == "<none>" {
            row.id.clone()
        } else {
            format!("{}:{}", row.repository, row.tag)
        };
        let in_use = image_in_use(&row.repository, &row.tag, &row.id);
        let delete_btn = if in_use {
            r#"<span class="muted" title="In use by a container">In use</span>"#.to_string()
        } else {
            format!(
                r#"<form method="post" action="/docker/images/delete" class="inline-form" style="display:inline;" onsubmit="return confirm('Delete image {label}?');">
              <input type="hidden" name="repository" value="{repo}">
              <input type="hidden" name="tag" value="{tag}">
              <input type="hidden" name="id" value="{id}">
              <button type="submit" class="btn-danger" title="Delete">Delete</button>
            </form>"#,
                label = html_escape(&ref_name),
                repo = html_escape(&row.repository),
                tag = html_escape(&row.tag),
                id = html_escape(&row.id),
            )
        };
        body.push_str(&format!(
            r#"<tr>
          <td data-label="Image name"><code>{repo}</code><br><code class="muted">{id}</code></td>
          <td data-label="Tags"><code>{tag}</code></td>
          <td data-label="Size">{size}</td>
          <td data-label="Actions" class="docker-actions">
            <form method="post" action="/docker/images/pull" class="inline-form" style="display:inline;">
              <input type="hidden" name="image" value="{ref_name}">
              <button type="submit" class="btn-primary" title="Pull / update this tag">Pull</button>
            </form>
            <a class="btn-secondary" href="/docker/create?image={enc}">Create</a>
            {delete}
          </td>
        </tr>"#,
            repo = html_escape(&row.repository),
            id = html_escape(&row.id),
            tag = html_escape(&row.tag),
            size = html_escape(&row.size),
            ref_name = html_escape(&ref_name),
            enc = urlencoding_simple(&ref_name),
            delete = delete_btn,
        ));
    }
    body.push_str("</tbody></table></div>");
    body
}

fn search_pull_card(query: &str, hits: &[DockerHubSearchHit]) -> String {
    let q = html_escape(query);
    format!(
        r#"<div class="panel-card" style="margin:18px 0;">
      <h2 style="margin:0 0 8px;font-size:18px;">Search &amp; Pull Images</h2>
      <p class="muted" style="margin:0 0 12px;">Search Docker Hub for official or maintainer images (Official badge first), pull, then run as a container or <a href="/docker/stacks">compose stack</a> with host volumes.</p>
      <form method="get" action="/docker/images" class="docker-hub-form" style="grid-template-columns:1fr auto;align-items:end;margin-bottom:12px;">
        <label>Search Docker Hub
          <input type="search" name="q" value="{q}" maxlength="100" placeholder="nginx, mariadb, redis" autocomplete="off">
        </label>
        <button type="submit" class="btn-secondary">Search</button>
      </form>
      <form method="post" action="/docker/images/pull" class="docker-hub-form" style="grid-template-columns:1fr auto;align-items:end;">
        <label>Image to pull
          <input type="text" name="image" required maxlength="255" placeholder="nginx:alpine (Docker Official Image)" value="{q}" autocomplete="off">
        </label>
        <button type="submit" class="btn-primary">Pull</button>
      </form>
      {results}
    </div>"#,
        q = q,
        results = search_results_html(hits),
    )
}

pub fn docker_images_page(
    notice: Option<&str>,
    error: Option<&str>,
    query: Option<&str>,
    hits: &[DockerHubSearchHit],
    list_query: Option<&DockerListQuery>,
) -> String {
    let list_query = list_query.cloned().unwrap_or(DockerListQuery {
        q: String::new(),
        page: 1,
        per_page: 10,
    });
    let status = docker_status();
    if !status.installed {
        return feature_shell(
            &[
                ("Dashboard", Some("/dashboard")),
                ("Server", Some("/server")),
                ("Docker", None),
            ],
            "Docker",
            "Container engine for this CPN host.",
            &format!(
                "{}{}",
                crate::panel_hubs::not_configured_body(
                    &status.detail,
                    "Install the Docker host package from Plugins > Store (search docker), or run cpn app install --name docker."
                ),
                r#"<p style="margin-top:12px;"><a class="btn-primary" href="/plugins?view=store&amp;target=host&amp;q=docker">Open Store</a></p>"#
            ),
            notice,
            error,
        );
    }
    let q = query.unwrap_or("");
    let hub_extra = if q.is_empty() {
        String::new()
    } else {
        format!("q={}", urlencoding_simple(q))
    };
    let (table, list_bar) = match list_images_detailed() {
        Ok(rows) => {
            let filtered = filter_images(&rows, &list_query.q);
            let owned: Vec<DockerImageRow> = filtered.into_iter().cloned().collect();
            let total = owned.len();
            let (page_rows, total_pages) =
                paginate_slice(&owned, list_query.page, list_query.per_page);
            let refs: Vec<&DockerImageRow> = page_rows.iter().collect();
            let empty = if rows.is_empty() {
                "No images on this host. Search Docker Hub above or pull by name (for example nginx:alpine)."
            } else {
                "No local images match this filter."
            };
            let bar = docker_list_toolbar(
                "/docker/images",
                "lq",
                "repository, tag, id",
                &list_query.q,
                list_query.page.min(total_pages),
                list_query.per_page,
                total_pages,
                total,
                &hub_extra,
            );
            (images_table(&refs, empty), bar)
        }
        Err(e) => (
            format!(
                r#"<p class="panel-notice error" role="status">{}</p>"#,
                html_escape(&e)
            ),
            String::new(),
        ),
    };
    let body = format!(
        r##"{toolbar}
      <p class="stack-actions" style="margin:16px 0;display:flex;flex-wrap:wrap;gap:8px;">
        <a class="btn-primary" href="/docker/create">+ Create Container</a>
        <a class="btn-secondary" href="/docker/list">Active Containers</a>
      </p>
      {search}
      <div class="panel-card" style="margin:18px 0;">
        <div style="display:flex;flex-wrap:wrap;align-items:center;justify-content:space-between;gap:10px;margin-bottom:12px;">
          <h2 style="margin:0;font-size:18px;">Local Images</h2>
          <form method="post" action="/docker/images/prune" onsubmit="return confirm('Prune all unused images on this host?');">
            <button type="submit" class="btn-secondary" style="background:#c2410c;border-color:#c2410c;color:#fff;">Prune Unused</button>
          </form>
        </div>
        {list_bar}
        {table}
      </div>"##,
        toolbar = toolbar("images"),
        search = search_pull_card(q, hits),
        list_bar = list_bar,
        table = table,
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Server", Some("/server")),
            ("Docker", Some("/docker")),
            ("Images", None),
        ],
        "Docker Images",
        "Pull, manage, and organize images for your containers.",
        &body,
        notice,
        error,
    )
}
