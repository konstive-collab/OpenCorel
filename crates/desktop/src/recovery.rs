//! Crash recovery snapshots and diagnostic reports for the desktop editor.
//! Recovery files are independent, validated KDR containers written atomically.
use opencorel_core::{kdr,Document};
use std::{
    env,fs,
    path::{Path,PathBuf},
    time::{Duration,SystemTime,UNIX_EPOCH},
};

const RECOVERY_RETENTION:Duration=Duration::from_secs(30*24*60*60);

#[derive(Debug)]
pub(super) struct RecoveryCandidate{
    pub path:PathBuf,
    pub document:Document,
    pub modified:SystemTime,
}

pub(super) fn recovery_dir()->PathBuf{
    #[cfg(target_os="windows")]
    {
        let base=env::var_os("LOCALAPPDATA")
            .or_else(||env::var_os("APPDATA"))
            .map(PathBuf::from).unwrap_or_else(env::temp_dir);
        return base.join("OpenCorel").join("Recovery");
    }
    #[cfg(target_os="macos")]
    {
        let base=env::var_os("HOME").map(PathBuf::from)
            .unwrap_or_else(env::temp_dir);
        return base.join("Library").join("Application Support")
            .join("OpenCorel").join("Recovery");
    }
    #[cfg(not(any(target_os="windows",target_os="macos")))]
    {
        let base=env::var_os("XDG_STATE_HOME").map(PathBuf::from)
            .or_else(||env::var_os("HOME").map(|home|
                PathBuf::from(home).join(".local").join("state")))
            .unwrap_or_else(env::temp_dir);
        base.join("opencorel").join("recovery")
    }
}

pub(super) fn new_session_path()->PathBuf{
    let stamp=SystemTime::now().duration_since(UNIX_EPOCH)
        .unwrap_or_default().as_nanos();
    recovery_dir().join(format!("session-{}-{stamp}.kdr",std::process::id()))
}

pub(super) fn save_snapshot(document:&Document,path:&Path)->Result<(),String>{
    document.validate()?;
    let parent=path.parent().ok_or("Recovery path has no parent directory")?;
    fs::create_dir_all(parent).map_err(|e|format!("Cannot create recovery directory: {e}"))?;
    kdr::save_kdr(document,path)
}

pub(super) fn discard(path:&Path){
    let _=fs::remove_file(path);
}

/// Return the newest valid recovery snapshot from another editor session.
/// Corrupt snapshots are ignored and removed; stale files are pruned.
pub(super) fn latest(exclude:&Path)->Option<RecoveryCandidate>{
    let dir=recovery_dir();
    let entries=fs::read_dir(&dir).ok()?;
    let now=SystemTime::now();
    let mut candidates=Vec::new();
    for entry in entries.flatten(){
        let path=entry.path();
        if path==exclude||path.extension().and_then(|e|e.to_str())!=Some("kdr")
            ||!path.file_name().and_then(|n|n.to_str())
                .is_some_and(|n|n.starts_with("session-")){
            continue;
        }
        let Ok(metadata)=entry.metadata() else{continue;};
        let modified=metadata.modified().unwrap_or(UNIX_EPOCH);
        if now.duration_since(modified).unwrap_or_default()>RECOVERY_RETENTION{
            discard(&path);continue;
        }
        candidates.push((path,modified));
    }
    candidates.sort_by(|a,b|b.1.cmp(&a.1));
    for (path,modified) in candidates{
        match kdr::read_kdr(&path){
            Ok(document)=>return Some(RecoveryCandidate{path,document,modified}),
            Err(_)=>discard(&path),
        }
    }
    None
}

/// Install a small crash report hook. The normal Rust panic hook is preserved,
/// and a report never includes document contents or embedded artwork.
pub(super) fn install_panic_hook(){
    let previous=std::panic::take_hook();
    std::panic::set_hook(Box::new(move|info|{
        let dir=recovery_dir();
        let stamp=SystemTime::now().duration_since(UNIX_EPOCH)
            .unwrap_or_default().as_secs();
        let path=dir.join(format!("crash-{}-{stamp}.log",std::process::id()));
        let message=info.payload().downcast_ref::<&str>().copied()
            .or_else(||info.payload().downcast_ref::<String>().map(String::as_str))
            .unwrap_or("non-string panic payload");
        let location=info.location().map(|l|
            format!("{}:{}:{}",l.file(),l.line(),l.column()))
            .unwrap_or_else(||"unknown location".into());
        let report=format!(
            "OpenCorel crash report\nVersion: {}\nTime: {:?}\nLocation: {}\nPanic: {}\n",
            env!("CARGO_PKG_VERSION"),SystemTime::now(),location,message
        );
        if fs::create_dir_all(&dir).is_ok(){let _=fs::write(path,report);}
        previous(info);
    }));
}

#[cfg(test)]
mod tests{
    use super::*;
    use opencorel_core::{Shape,Style};

    #[test]
    fn recovery_snapshot_is_a_valid_roundtrip(){
        let dir=tempfile::tempdir().unwrap();
        let path=dir.path().join("session-test.kdr");
        let mut doc=Document::default();
        doc.add(Shape::Rectangle{x:10.,y:20.,w:30.,h:40.},Style::default());
        save_snapshot(&doc,&path).unwrap();
        assert_eq!(kdr::read_kdr(&path).unwrap(),doc);
    }

    #[test]
    fn failed_recovery_save_does_not_replace_existing_snapshot(){
        let dir=tempfile::tempdir().unwrap();
        let path=dir.path().join("session-test.kdr");
        let good=Document::default();
        save_snapshot(&good,&path).unwrap();
        let mut invalid=good.clone();
        invalid.page.width_mm=f32::NAN;
        assert!(save_snapshot(&invalid,&path).is_err());
        assert_eq!(kdr::read_kdr(&path).unwrap(),good);
    }

    #[test]
    fn recovery_directory_is_platform_specific_and_stable(){
        let a=recovery_dir();
        let b=recovery_dir();
        assert_eq!(a,b);
        assert!(a.to_string_lossy().to_lowercase().contains("opencorel"));
    }
}
