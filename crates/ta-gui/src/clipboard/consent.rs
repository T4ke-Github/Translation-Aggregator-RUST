use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{self, BufRead, Read, Write},
    path::Path,
};

use super::provider::ClipboardProvider;

const MAX_APPROVAL_BYTES: u64 = 128 * 1024;

pub(super) fn configure(
    providers: &mut [ClipboardProvider],
    data_dir: &Path,
    mut confirm: impl FnMut(&str, &str, &str) -> io::Result<bool>,
) -> io::Result<()> {
    for provider in providers.iter_mut() {
        provider.approved_origin = None;
    }
    let path = data_dir.join("remote-auto-consent.json");
    if !path.try_exists()?
        && providers
            .iter()
            .all(|provider| provider.consent_origin().is_none())
    {
        return Ok(());
    }
    fs::create_dir_all(data_dir)?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(data_dir.join("remote-auto-consent.lock"))?;
    lock.try_lock().map_err(io::Error::from)?;

    let saved = read_approvals(&path)?;
    let mut approvals = BTreeMap::new();
    for provider in providers.iter() {
        let Some(origin) = provider.consent_origin() else {
            continue;
        };
        let definition = &provider.provider.definition().provider;
        if saved
            .get(&definition.id)
            .is_some_and(|saved| saved == origin)
            || confirm(&definition.name, &definition.id, origin)?
        {
            approvals.insert(definition.id.clone(), origin.to_owned());
        }
    }
    if approvals != saved {
        let bytes = serde_json::to_vec_pretty(&approvals).map_err(io::Error::other)?;
        if bytes.len() as u64 > MAX_APPROVAL_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "remote auto consent is too large",
            ));
        }
        ta_platform::atomic_save(&path, &bytes)?;
    }
    for provider in providers {
        if let Some(origin) = provider.consent_origin()
            && approvals
                .get(&provider.provider.definition().provider.id)
                .is_some_and(|approved| approved == origin)
        {
            provider.approved_origin = Some(origin.to_owned());
        }
    }
    Ok(())
}

fn read_approvals(path: &Path) -> io::Result<BTreeMap<String, String>> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(error) => return Err(error),
    };
    let mut bytes = Vec::new();
    file.take(MAX_APPROVAL_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_APPROVAL_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "remote auto consent is too large",
        ));
    }
    serde_json::from_slice(&bytes).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid remote auto consent file",
        )
    })
}

pub(super) fn prompt(
    input: &mut impl BufRead,
    output: &mut impl Write,
    name: &str,
    id: &str,
    origin: &str,
) -> io::Result<bool> {
    writeln!(
        output,
        "Allow automatic translation for {name:?} ({id:?}) at {origin}?\nClipboard text leaves this machine and requests may cost money.\nType yes to allow, or press Enter to cancel."
    )?;
    output.flush()?;
    let mut reply = String::new();
    input.read_line(&mut reply)?;
    Ok(reply.trim() == "yes")
}
