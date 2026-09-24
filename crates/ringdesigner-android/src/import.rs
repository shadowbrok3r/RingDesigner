//! Parts brought in from files as the desktop's File ▸ Import part does: STL and OBJ by the solid crate's readers, STEP's faceted solids by the core's,
//! found in the app's own folders and in Downloads, read off the UI thread.
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::time::SystemTime;

use egui_mobile::egui;
use ringdesign_core::cad::{Feature, step, stored};

/// The extensions a part file may carry.
pub const EXTENSIONS: [&str; 4] = ["stl", "obj", "step", "stp"];
/// The shared Downloads folder, where a file the app shares or saves lands.
pub const DOWNLOADS: &str = "/storage/emulated/0/Download";
/// The most part files one folder offers, newest first.
const MAX_LISTED: usize = 40;
/// The largest part file the phone reads.
pub const MAX_IMPORT_BYTES: u64 = 32 * 1024 * 1024;

/// A part file found in one of the folders looked in.
#[derive(Clone, Debug, PartialEq)]
pub struct PartFile {
    pub path: PathBuf,
    pub name: String,
    /// The folder it was found in, as the list names it.
    pub folder: &'static str,
    pub bytes: u64,
    pub modified: Option<SystemTime>,
}

/// Whether `path` names a part file by its extension.
pub fn is_part(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| EXTENSIONS.iter().any(|x| e.eq_ignore_ascii_case(x)))
}

/// The part files in `folders`, each folder's newest first, hidden and empty files left out; a folder that cannot be read offers none.
pub fn candidates(folders: &[(&'static str, PathBuf)]) -> Vec<PartFile> {
    let mut out = Vec::new();
    for (folder, dir) in folders {
        let Ok(entries) = std::fs::read_dir(dir) else { continue };
        let mut found: Vec<PartFile> = entries
            .flatten()
            .filter_map(|e| {
                let path = e.path();
                let name = e.file_name().to_string_lossy().into_owned();
                let meta = e.metadata().ok().filter(|m| m.is_file() && m.len() > 0)?;
                (is_part(&path) && !name.starts_with('.')).then(|| PartFile { name, folder, bytes: meta.len(), modified: meta.modified().ok(), path })
            })
            .collect();
        found.sort_by(|a, b| b.modified.cmp(&a.modified).then_with(|| a.name.cmp(&b.name)));
        found.truncate(MAX_LISTED);
        out.extend(found);
    }
    out
}

/// The part `path` holds as a stored feature joined at the top of the ring, and what reading it said; refused in words naming the file.
pub fn read(path: &Path) -> Result<(Feature, Vec<String>), String> {
    let file = path.file_name().and_then(|n| n.to_str()).unwrap_or("The file").to_string();
    if let Some(why) = std::fs::metadata(path).ok().and_then(|m| too_big(&file, m.len())) {
        return Err(why);
    }
    let ext = path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).unwrap_or_default();
    let (format, solids, notes) = match ext.as_str() {
        "stl" => ("stl", vec![ringdesign_solid::io::read_stl(path).map_err(|e| format!("{file} does not read as STL: {e:#}"))?], Vec::new()),
        "obj" => ("obj", vec![ringdesign_solid::io::read_obj(path).map_err(|e| format!("{file} does not read as OBJ: {e:#}"))?], Vec::new()),
        "step" | "stp" => {
            let text = std::fs::read_to_string(path).map_err(|e| format!("{file} could not be read: {e}"))?;
            let (meshes, notes) = step::solid_meshes(&text, &file).map_err(|e| format!("{e:#}"))?;
            ("step", meshes, notes)
        }
        _ => return Err(format!("{file}: a part comes in as STL, OBJ or STEP")),
    };
    let feature = stored::imported(&file, format, &solids).map_err(|e| format!("{e:#}"))?;
    Ok((feature, notes))
}

/// Why a part file `bytes` long is not read, or `None` when it is within the phone's size.
pub fn too_big(file: &str, bytes: u64) -> Option<String> {
    let mb = |b: u64| b as f64 / 1048576.0;
    (bytes > MAX_IMPORT_BYTES).then(|| format!("{file} is {:.0} MB; the phone reads parts up to {:.0} MB", mb(bytes), mb(MAX_IMPORT_BYTES)))
}

/// What reading a part file came to.
pub struct Read {
    pub path: PathBuf,
    pub part: Result<(Feature, Vec<String>), String>,
}

/// Reads `path` on its own thread; the receiver yields once.
pub fn spawn(path: PathBuf, ctx: egui::Context) -> Receiver<Read> {
    let (tx, rx) = channel();
    std::thread::Builder::new()
        .name("ring-import".into())
        .spawn(move || {
            let part = read(&path);
            let _ = tx.send(Read { path, part });
            ctx.request_repaint();
        })
        .expect("spawn import thread");
    rx
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringdesign_core::{
        AlphaLibrary, BuildParams, Mesh, RingDesign, Vec3,
        cad::{Attach, Operation, Placement},
        history::History,
        mesh, templates,
    };

    /// A scratch folder of this test's own.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rd-import-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A 2 mm cube standing on z = `foot`, wound outward.
    fn cube(foot: f32) -> Mesh {
        let (a, b) = (foot, foot + 2.0);
        let vertices = [[-1.0, -1.0, a], [1.0, -1.0, a], [1.0, 1.0, a], [-1.0, 1.0, a], [-1.0, -1.0, b], [1.0, -1.0, b], [1.0, 1.0, b], [-1.0, 1.0, b]].map(|p| Vec3(p[0], p[1], p[2])).to_vec();
        let faces = vec![[0, 2, 1], [0, 3, 2], [4, 5, 6], [4, 6, 7], [0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6], [3, 0, 4], [3, 4, 7]];
        Mesh { vertices, faces, ..Mesh::default() }
    }

    fn court() -> RingDesign {
        templates::all().iter().find(|t| t.name == "Court band").unwrap().design()
    }

    #[test]
    fn a_part_file_is_read_by_its_kind_and_one_that_does_not_close_is_refused_by_name() {
        let dir = scratch("read");
        let shape = cube(0.0);
        assert!((shape.volume_mm3() - 8.0).abs() < 1e-9, "the cube is wound outward");
        let stl = dir.join("post.stl");
        ringdesign_core::stl::write_stl(&stl, &shape, "post").unwrap();
        let (feature, notes) = read(&stl).unwrap();
        assert_eq!((feature.name.as_str(), notes.len()), ("post", 0));
        assert!(matches!(&feature.operation, Operation::Stored { recipe, sources, .. } if recipe.op == "import" && sources.is_empty()), "{:?}", feature.operation);
        assert_eq!((feature.component.attach, feature.component.placement.clone()), (Attach::Join, Placement::ring(ringdesign_core::profile::TOP_DEG, 0.0)));
        let obj = dir.join("Block.OBJ");
        ringdesign_core::stl::write_obj(&obj, &shape, "block").unwrap();
        assert_eq!(read(&obj).unwrap().0.name, "Block", "an OBJ, its extension in capitals");
        // A STEP of a ring with a post: the band's faceted solid and the post's exact one both come in.
        let mut d = court();
        let (edits, _) = ringdesign_workbench::touch::parts::part_here(&d, "Cylinder", 90.0, 0.0).unwrap();
        d = ringdesign_workbench::touch::prepare(&d, &edits, None).unwrap().unwrap().design;
        let text = step::ring(&d, &AlphaLibrary::builtin(), BuildParams { theta_steps: 96, profile_steps: 48, ..BuildParams::default() }, "court").unwrap();
        let step_file = dir.join("court.step");
        std::fs::write(&step_file, &text).unwrap();
        let (ring, notes) = read(&step_file).unwrap();
        assert_eq!((ring.name.as_str(), notes.len()), ("court", 0), "{notes:?}");
        let Operation::Stored { mesh, .. } = &ring.operation else { panic!("{:?}", ring.operation) };
        let whole: f64 = step::read_meshes(&text).unwrap().iter().map(|m| m.mesh.as_ref().unwrap().volume_mm3()).sum();
        let packed = mesh.made().unwrap().solid().volume();
        assert!((packed - whole).abs() < 5e-3 * whole, "{packed} against {whole}");
        // A cube with its lid missing does not close, and a file of another kind is refused.
        let mut open = shape.clone();
        open.faces.truncate(10);
        let lidless = dir.join("lidless.stl");
        ringdesign_core::stl::write_stl(&lidless, &open, "lidless").unwrap();
        let why = read(&lidless).unwrap_err();
        assert!(why.starts_with("lidless.stl is not a closed solid"), "{why}");
        assert_eq!(read(&dir.join("notes.txt")).unwrap_err(), "notes.txt: a part comes in as STL, OBJ or STEP");
        // A file past the phone's size is refused before a byte of it is read.
        let big = dir.join("ring.step");
        std::fs::File::create(&big).unwrap().set_len(227_548_616).unwrap();
        assert_eq!(read(&big).unwrap_err(), "ring.step is 217 MB; the phone reads parts up to 32 MB");
        assert_eq!(too_big("post.stl", MAX_IMPORT_BYTES), None);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_list_offers_each_folders_part_files_newest_first_and_skips_what_is_not_one() {
        let (a, b) = (scratch("list-a"), scratch("list-b"));
        for (dir, name) in [(&a, "old.stl"), (&a, "notes.txt"), (&b, "ring.step"), (&b, "part.stp"), (&b, ".pending-1790816067-ring.step")] {
            std::fs::write(dir.join(name), b"x").unwrap();
        }
        // A file MediaStore is still writing is hidden and may be empty; neither is offered.
        std::fs::write(b.join("empty.stl"), b"").unwrap();
        std::fs::create_dir_all(a.join("folder.stl")).unwrap();
        // Written a moment later, so it is the newer of the two in its folder.
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(a.join("new.OBJ"), b"x").unwrap();
        let found = candidates(&[("App imports", a.clone()), ("Nowhere", a.join("missing")), ("Downloads", b.clone())]);
        let listed: Vec<(&str, &str)> = found.iter().map(|f| (f.folder, f.name.as_str())).collect();
        assert_eq!(listed, [("App imports", "new.OBJ"), ("App imports", "old.stl"), ("Downloads", "part.stp"), ("Downloads", "ring.step")]);
        assert!(found.iter().all(|f| f.bytes == 1));
        let _ = std::fs::remove_dir_all(a);
        let _ = std::fs::remove_dir_all(b);
    }

    #[test]
    fn an_imported_part_stands_at_the_top_of_the_ring_joined_as_one_undo_step() {
        let dir = scratch("join");
        let stl = dir.join("post.stl");
        ringdesign_core::stl::write_stl(&stl, &cube(-0.3), "post").unwrap();
        let rx = spawn(stl.clone(), egui::Context::default());
        let got = rx.recv().unwrap();
        assert_eq!(got.path, stl);
        let (feature, _) = got.part.unwrap();
        let mut d = court();
        let mut history = History::new(&d);
        let edits = stored::import_edits(&d, feature);
        let done = crate::cad::commit(&mut d, &mut history, &edits, None).unwrap().unwrap();
        assert_eq!(done.label, "Add Procedural shank · Add post");
        assert_eq!(history.timeline().len(), 2, "one undo step");
        let params = BuildParams { theta_steps: 192, profile_steps: 96, refine: None, ..BuildParams::default() };
        let before = mesh::build(&court(), &AlphaLibrary::builtin(), params).mesh.volume_mm3();
        let after = mesh::build(&d, &AlphaLibrary::builtin(), params);
        assert_eq!(after.parts.joined, 1, "{:?}", after.parts.notes);
        assert!(after.report.validation.watertight);
        // Its highest point stands 1.7 mm over the crest at the top of the ring.
        let post = after.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.name == "post").unwrap();
        let (_, hi) = post.mesh.bounds().unwrap();
        let crest = before_crest(&params);
        assert!((f64::from(hi.1) - crest - 1.7).abs() < 0.01, "{} over a crest at {crest}", hi.1);
        // The 8 mm³ cube less what of its foot lies inside the band under the crest.
        let grew = after.mesh.volume_mm3() - before;
        assert!((grew - 7.147).abs() < 0.01, "the cube sunk 0.3 mm into the crest grows the ring by {grew:.4} mm³");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Where the bare Court band's surface stands at the top of the ring.
    fn before_crest(params: &BuildParams) -> f64 {
        let built = mesh::build(&court(), &AlphaLibrary::builtin(), *params);
        ringdesign_core::cad::surface_hit(&built.mesh, ringdesign_core::profile::TOP_DEG, 0.0).unwrap().0[1]
    }
}
