use crate::toast::{breadbox::ImportMap, cache::init, node::render_to_html};
use color_eyre::{
    eyre::{eyre, Report, Result, WrapErr},
    Section,
};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

pub struct IncrementalOpts {
    pub debug: bool,
    pub project_root_dir: PathBuf,
    pub output_dir: PathBuf,
    pub npm_bin_dir: String,
    pub import_map: ImportMap,
}

#[derive(Debug)]
struct OutputFile {
    dest: String,
}

pub fn incremental_compile(
    IncrementalOpts {
        debug,
        project_root_dir,
        output_dir,
        npm_bin_dir,
        import_map,
    }: IncrementalOpts,
) -> Result<()> {
    let tmp_dir = {
        let mut dir = project_root_dir.clone();
        dir.push(".tmp");
        dir
    };
    std::fs::create_dir_all(&tmp_dir).wrap_err_with(|| {
        format!(
            "Failed to create directories for tmp_dir `{}`. Can not compile files into directory that doesn't exist, exiting.",
            &tmp_dir.display()
        )
    })?;

    let mut cache = init(npm_bin_dir.clone());
    let files_by_source_id: HashMap<String, OutputFile> =
        WalkDir::new(&project_root_dir.join("src"))
            .into_iter()
            .filter(|result| {
                return result.as_ref().map_or(false, |dir_entry| {
                    dir_entry
                        .file_name()
                        .to_str()
                        .map(|filename| filename.ends_with(".js"))
                        .unwrap_or(false)
                });
            })
            // insert source files into cache and return a
            // HashMap so we can access the entries and such later
            // by source_id
            .fold(HashMap::new(), |mut map, entry| {
                let e = entry.unwrap();
                let file_stuff = std::fs::read(e.path()).unwrap();
                let source_id = e
                    .path()
                    .strip_prefix(&project_root_dir)
                    .unwrap()
                    .to_str()
                    .unwrap();
                cache.set_source(source_id, file_stuff);
                map.entry(String::from(source_id)).or_insert(OutputFile {
                    dest: source_id.to_string(),
                });
                map
            });

    for (source_id, output_file) in files_by_source_id.iter() {
        let browser_output_file = output_dir.join(Path::new(&output_file.dest));
        let js_browser = cache.get_js_for_browser(source_id, import_map.clone());
        std::fs::create_dir_all(&browser_output_file.parent().unwrap());
        let _res = std::fs::write(&browser_output_file, js_browser).wrap_err_with(|| {
            format!(
                "Failed to write browser JS file for `{}`. ",
                &browser_output_file.display()
            )
        })?;

        let js_node = cache.get_js_for_server(source_id);
        let mut node_output_file = tmp_dir.clone();
        node_output_file.push(&output_file.dest);
        node_output_file.set_extension("mjs");
        // TODO: handle directory creation errors gracefully
        std::fs::create_dir_all(&node_output_file.parent().unwrap());
        let _node_res = std::fs::write(&node_output_file, js_node).wrap_err_with(|| {
            format!(
                "Failed to write node JS file for `{}`. ",
                &node_output_file.display()
            )
        })?;
    }

    let file_list = files_by_source_id
        .iter()
        .map(|(_, output_file)| output_file.dest.clone())
        .collect::<Vec<String>>();
    render_to_html(
        tmp_dir.into_os_string().into_string().unwrap(),
        output_dir.into_os_string().into_string().unwrap(),
        file_list,
        npm_bin_dir,
    );
    Ok(())
}