// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Isolated encrypted Secret Service record contracts.

#![cfg(target_os = "linux")]

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::num::NonZeroU32;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tallyline_adapter_platform::api::{LinuxCredentialBus, LinuxCredentialStore};
use tallyline_shared_kernel::api::model::Timestamp;
use tallyline_submission::api::model::{
    CredentialCandidate, CredentialValues, Persistence, StorageFailure, ValidatedAccount,
};
use tallyline_submission::api::{CredentialApplication, SecretStore};
use zbus::zvariant::{
    Endian, OwnedObjectPath, OwnedValue, Type, Value,
    serialized::{Context, Data},
};

const ITEM: &str = "/org/freedesktop/secrets/collection/dummy/item";
const COLLECTION: &str = "/org/freedesktop/secrets/collection/dummy";
const SESSION: &str = "/org/freedesktop/secrets/session/dummy";
const BUNDLE: &[u8] = b"TLCR\x01\x09\x00\x0b\x00\x0c\x00\x39\x30\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00dummy-keydummy-tokendummy-runner";
type Error = Box<dyn std::error::Error + Send + Sync>;
type Header = (u8, u8, u8, u8, u32, u32, HashMap<u8, OwnedValue>);

#[derive(Clone, Serialize, Deserialize, Type)]
struct Secret(OwnedObjectPath, Vec<u8>, Vec<u8>, String);

struct Call {
    serial: NonZeroU32,
    member: String,
    body: Vec<u8>,
}

impl Call {
    fn read(stream: &mut impl Read) -> Result<Option<Self>, Error> {
        let mut header = [0; 16];
        match stream.read_exact(&mut header) {
            Ok(()) => (),
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(error) => return Err(error.into()),
        }
        assert_eq!(header[0], b'l');
        let body_length = u32::from_le_bytes(header[4..8].try_into()?) as usize;
        let serial =
            NonZeroU32::new(u32::from_le_bytes(header[8..12].try_into()?)).ok_or("zero serial")?;
        let fields_length = u32::from_le_bytes(header[12..16].try_into()?) as usize;
        let mut fields = vec![0; fields_length.next_multiple_of(8)];
        stream.read_exact(&mut fields)?;
        let mut bytes = header.to_vec();
        bytes.extend_from_slice(&fields);
        let data = Data::new(bytes, Context::new_dbus(Endian::Little, 0));
        let ((_, _, _, _, _, _, fields), _) = data.deserialize::<Header>()?;
        let member = fields.get(&3).ok_or("missing member")?;
        let member = <&str>::try_from(member)?.to_owned();
        let mut body = vec![0; body_length];
        stream.read_exact(&mut body)?;
        Ok(Some(Self {
            serial,
            member,
            body,
        }))
    }

    fn body<T: serde::de::DeserializeOwned + Type>(&self) -> Result<T, Error> {
        Ok(
            Data::new(self.body.as_slice(), Context::new_dbus(Endian::Little, 0))
                .deserialize::<T>()?
                .0,
        )
    }

    fn reply<T: Serialize + zbus::zvariant::DynamicType>(
        &self,
        stream: &mut impl Write,
        body: &T,
    ) -> Result<(), Error> {
        let call = zbus::Message::method_call("/", "Dummy")?
            .serial(self.serial)
            .build(&())?;
        let reply = zbus::Message::method_return(&call.header())?
            .sender(":1.2")?
            .build(body)?;
        stream.write_all(reply.data())?;
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum Fault {
    None,
    Locked,
    Ambiguous,
    Corrupt,
    Write,
    NoCollection,
    DhRefused,
    CommitWithoutReply,
}

fn serve(
    listener: UnixListener,
    fault: Arc<Mutex<Fault>>,
) -> std::thread::JoinHandle<Result<(), Error>> {
    std::thread::spawn(move || {
        let (stream, _) = listener.accept()?;
        stream.set_read_timeout(Some(Duration::from_secs(10)))?;
        let mut stream = BufReader::new(stream);
        let mut line = String::new();
        stream.read_line(&mut line)?;
        assert!(line.starts_with("\0AUTH EXTERNAL "));
        stream
            .get_mut()
            .write_all(b"OK 0123456789abcdef0123456789abcdef\r\n")?;
        line.clear();
        stream.read_line(&mut line)?;
        assert_eq!(line, "NEGOTIATE_UNIX_FD\r\n");
        stream.get_mut().write_all(b"AGREE_UNIX_FD\r\n")?;
        line.clear();
        stream.read_line(&mut line)?;
        assert_eq!(line, "BEGIN\r\n");
        let mut stored: Option<Secret> = None;
        let mut lost_save = false;
        let mut lost_delete = false;
        while let Some(call) = Call::read(&mut stream)? {
            let fault = *fault.lock().map_err(|_| "fixture lock")?;
            match call.member.as_str() {
                "Hello" => call.reply(stream.get_mut(), &":1.1")?,
                "GetNameOwner" => call.reply(stream.get_mut(), &":1.2")?,
                "AddMatch" | "RemoveMatch" => call.reply(stream.get_mut(), &())?,
                "OpenSession" => {
                    let (algorithm, public): (String, OwnedValue) = call.body()?;
                    assert_eq!(algorithm, "dh-ietf1024-sha256-aes128-cbc-pkcs7");
                    assert!(!Vec::<u8>::try_from(public)?.is_empty());
                    if matches!(fault, Fault::DhRefused) {
                        let request = zbus::Message::method_call("/", "Dummy")?
                            .serial(call.serial)
                            .build(&())?;
                        let reply = zbus::Message::error(
                            &request.header(),
                            "org.freedesktop.DBus.Error.NotSupported",
                        )?
                        .build(&"dummy-private-error")?;
                        stream.get_mut().write_all(reply.data())?;
                        continue;
                    }
                    call.reply(
                        stream.get_mut(),
                        &(Value::from(vec![2u8]), OwnedObjectPath::try_from(SESSION)?),
                    )?;
                }
                "ReadAlias" => {
                    assert_eq!(call.body::<String>()?, "default");
                    call.reply(
                        stream.get_mut(),
                        &OwnedObjectPath::try_from(if matches!(fault, Fault::NoCollection) {
                            "/"
                        } else {
                            COLLECTION
                        })?,
                    )?;
                }
                "Get" => call.reply(
                    stream.get_mut(),
                    &Value::from(matches!(fault, Fault::Locked)),
                )?,
                "SearchItems" => {
                    assert_eq!(
                        call.body::<HashMap<String, String>>()?,
                        HashMap::from([
                            ("application".to_owned(), "Tallyline".to_owned()),
                            ("record".to_owned(), "uex-credentials".to_owned())
                        ])
                    );
                    let items = if stored.is_some() {
                        vec![OwnedObjectPath::try_from(ITEM)?]
                    } else {
                        vec![]
                    };
                    let items = if matches!(fault, Fault::Ambiguous) {
                        vec![
                            OwnedObjectPath::try_from(ITEM)?,
                            OwnedObjectPath::try_from(
                                "/org/freedesktop/secrets/collection/dummy/other",
                            )?,
                        ]
                    } else {
                        items
                    };
                    call.reply(stream.get_mut(), &items)?;
                }
                "CreateItem" => {
                    let (properties, secret, replace): (HashMap<String, OwnedValue>, Secret, bool) =
                        call.body()?;
                    assert!(replace);
                    assert_eq!(
                        <&str>::try_from(
                            properties
                                .get("org.freedesktop.Secret.Item.Label")
                                .ok_or("missing label")?
                        )?,
                        "Tallyline UEX credentials"
                    );
                    assert_eq!(secret.0.as_str(), SESSION);
                    assert_eq!(secret.1.len(), 16);
                    assert_ne!(secret.2, BUNDLE);
                    assert!(!secret.2.windows(9).any(|part| part == b"dummy-key"));
                    assert_eq!(secret.3, "application/octet-stream");
                    if matches!(fault, Fault::Write) {
                        let request = zbus::Message::method_call("/", "Dummy")?
                            .serial(call.serial)
                            .build(&())?;
                        let reply = zbus::Message::error(
                            &request.header(),
                            "org.freedesktop.DBus.Error.Failed",
                        )?
                        .build(&"dummy-private-error")?;
                        stream.get_mut().write_all(reply.data())?;
                        continue;
                    }
                    stored = Some(secret);
                    if matches!(fault, Fault::CommitWithoutReply) {
                        assert!(!lost_save);
                        lost_save = true;
                        continue;
                    }
                    call.reply(
                        stream.get_mut(),
                        &(
                            OwnedObjectPath::try_from(ITEM)?,
                            OwnedObjectPath::try_from("/")?,
                        ),
                    )?;
                }
                "SetSecret" => {
                    let (secret,): (Secret,) = call.body()?;
                    assert!(stored.is_some());
                    assert_eq!(secret.0.as_str(), SESSION);
                    assert!(!secret.2.windows(9).any(|part| part == b"dummy-key"));
                    stored = Some(secret);
                    if matches!(fault, Fault::CommitWithoutReply) {
                        assert!(!lost_save);
                        lost_save = true;
                        continue;
                    }
                    call.reply(stream.get_mut(), &())?;
                }
                "GetSecret" => {
                    let mut secret = stored.clone().ok_or("no record")?;
                    if matches!(fault, Fault::Corrupt) {
                        *secret.1.first_mut().ok_or("missing IV")? ^= 1;
                    }
                    call.reply(stream.get_mut(), &(secret,))?
                }
                "Delete" => {
                    stored = None;
                    if matches!(fault, Fault::CommitWithoutReply) {
                        assert!(!lost_delete);
                        lost_delete = true;
                        continue;
                    }
                    call.reply(stream.get_mut(), &OwnedObjectPath::try_from("/")?)?;
                }
                other => return Err(format!("unexpected method {other}").into()),
            }
        }
        Ok(())
    })
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_nf_4__encrypted_record_survives_save_load_replace_and_remove() {
    let directory = tempfile::tempdir().unwrap();
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let path = directory.path().join("bus");
    let server = serve(
        UnixListener::bind(&path).unwrap(),
        Arc::new(Mutex::new(Fault::None)),
    );
    let bus = LinuxCredentialBus::connect(&format!("unix:path={}", path.display()))
        .await
        .unwrap();
    let mut store = LinuxCredentialStore::connect(bus).await.unwrap();
    assert!(store.load().await.unwrap().is_none());
    let mut application = CredentialApplication::default();
    let request = application.begin(CredentialCandidate::new(
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap(),
        Persistence::Durable,
    ));
    application
        .accept(
            request,
            Ok(ValidatedAccount::new("dummy-runner".to_owned())),
            Timestamp::from_millis(12345),
            &mut store,
        )
        .await
        .unwrap();
    let saved = store.load().await.unwrap().unwrap();
    assert_eq!(saved.username(), "dummy-runner");
    assert_eq!(saved.values().secret_key(), "dummy-key");
    store.save(&saved).await.unwrap();
    assert_eq!(
        store.load().await.unwrap().unwrap().validated_at(),
        Timestamp::from_millis(12345)
    );
    store.remove().await.unwrap();
    store.remove().await.unwrap();
    assert!(store.load().await.unwrap().is_none());
    drop(store);
    server.join().unwrap().unwrap();
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_nf_4__locked_ambiguous_missing_and_corrupt_records_fail_closed_without_unlocking() {
    let directory = tempfile::tempdir().unwrap();
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let path = directory.path().join("bus");
    let fault = Arc::new(Mutex::new(Fault::None));
    let server = serve(UnixListener::bind(&path).unwrap(), fault.clone());
    let bus = LinuxCredentialBus::connect(&format!("unix:path={}", path.display()))
        .await
        .unwrap();
    let mut store = LinuxCredentialStore::connect(bus).await.unwrap();
    let saved = tallyline_submission::api::model::SavedCredentials::from_storage(
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap(),
        ValidatedAccount::new("dummy-runner".to_owned()),
        Timestamp::from_millis(12345),
        None,
    )
    .unwrap();
    store.save(&saved).await.unwrap();
    for (mode, expected) in [
        (Fault::Locked, StorageFailure::Locked),
        (Fault::Ambiguous, StorageFailure::Corrupt),
        (Fault::NoCollection, StorageFailure::Unavailable),
        (Fault::Corrupt, StorageFailure::Corrupt),
    ] {
        *fault.lock().unwrap() = mode;
        assert_eq!(store.load().await.err(), Some(expected));
        if matches!(mode, Fault::Locked | Fault::Ambiguous) {
            assert_eq!(store.save(&saved).await, Err(expected));
            assert_eq!(store.remove().await, Err(expected));
        }
    }
    *fault.lock().unwrap() = Fault::Write;
    assert_eq!(store.save(&saved).await, Err(StorageFailure::WriteFailed));
    *fault.lock().unwrap() = Fault::None;
    assert_eq!(
        store.load().await.unwrap().unwrap().username(),
        "dummy-runner"
    );
    drop(store);
    server.join().unwrap().unwrap();
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_nf_4__unsupported_dh_is_unavailable_without_plain_fallback() {
    let directory = tempfile::tempdir().unwrap();
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let path = directory.path().join("bus");
    let server = serve(
        UnixListener::bind(&path).unwrap(),
        Arc::new(Mutex::new(Fault::DhRefused)),
    );
    let bus = LinuxCredentialBus::connect(&format!("unix:path={}", path.display()))
        .await
        .unwrap();
    assert_eq!(
        LinuxCredentialStore::connect(bus).await.err(),
        Some(StorageFailure::Unavailable)
    );
    server.join().unwrap().unwrap();
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_nf_4__session_only_acceptance_remains_possible_while_the_store_is_locked() {
    let directory = tempfile::tempdir().unwrap();
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let path = directory.path().join("bus");
    let server = serve(
        UnixListener::bind(&path).unwrap(),
        Arc::new(Mutex::new(Fault::Locked)),
    );
    let bus = LinuxCredentialBus::connect(&format!("unix:path={}", path.display()))
        .await
        .unwrap();
    let mut store = LinuxCredentialStore::connect(bus).await.unwrap();
    assert_eq!(
        CredentialApplication::load(&mut store).await.err(),
        Some(StorageFailure::Locked)
    );
    let mut application = CredentialApplication::default();
    let request = application.begin(CredentialCandidate::new(
        CredentialValues::new("dummy-key".to_owned(), None).unwrap(),
        Persistence::Session,
    ));
    application
        .accept(
            request,
            Ok(ValidatedAccount::new("dummy-runner".to_owned())),
            Timestamp::from_millis(12345),
            &mut store,
        )
        .await
        .unwrap();
    assert!(matches!(
        application.state(),
        tallyline_submission::api::model::AccountState::Validated {
            persistence: Persistence::Session,
            ..
        }
    ));
    drop(store);
    server.join().unwrap().unwrap();
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_nf_4__committed_mutations_without_replies_are_uncertain_and_reconcile_without_retry() {
    let directory = tempfile::tempdir().unwrap();
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let path = directory.path().join("bus");
    let fault = Arc::new(Mutex::new(Fault::None));
    let server = serve(UnixListener::bind(&path).unwrap(), fault.clone());
    let bus = LinuxCredentialBus::connect(&format!("unix:path={}", path.display()))
        .await
        .unwrap();
    let mut store = LinuxCredentialStore::connect(bus).await.unwrap();
    let mut application = CredentialApplication::default();
    let request = application.begin(CredentialCandidate::new(
        CredentialValues::new("dummy-key".to_owned(), None).unwrap(),
        Persistence::Durable,
    ));
    application
        .accept(
            request,
            Ok(ValidatedAccount::new("dummy-original".to_owned())),
            Timestamp::from_millis(12345),
            &mut store,
        )
        .await
        .unwrap();
    *fault.lock().unwrap() = Fault::CommitWithoutReply;
    let request = application.begin(CredentialCandidate::new(
        CredentialValues::new("dummy-key".to_owned(), None).unwrap(),
        Persistence::Durable,
    ));
    assert_eq!(
        application
            .accept(
                request,
                Ok(ValidatedAccount::new("dummy-replacement".to_owned())),
                Timestamp::from_millis(23456),
                &mut store
            )
            .await,
        Err(
            tallyline_submission::api::model::CredentialFailure::Storage(
                StorageFailure::OutcomeUnknown
            )
        )
    );
    assert!(
        matches!(application.state(), tallyline_submission::api::model::AccountState::Validated { username, .. } if username == "dummy-original")
    );
    *fault.lock().unwrap() = Fault::None;
    let reconciled = store.load().await.unwrap().unwrap();
    assert_eq!(reconciled.username(), "dummy-replacement");
    assert_eq!(reconciled.validated_at(), Timestamp::from_millis(23456));
    *fault.lock().unwrap() = Fault::CommitWithoutReply;
    assert_eq!(store.remove().await, Err(StorageFailure::OutcomeUnknown));
    *fault.lock().unwrap() = Fault::None;
    assert!(store.load().await.unwrap().is_none());
    drop(store);
    server.join().unwrap().unwrap();
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_api_7_r_nf_4_r_ui_16__startup_revalidation_updates_existing_encrypted_record_without_prompt_or_creation()
 {
    let directory = tempfile::tempdir().unwrap();
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let path = directory.path().join("bus");
    let fault = Arc::new(Mutex::new(Fault::None));
    let server = serve(UnixListener::bind(&path).unwrap(), fault.clone());
    let bus = LinuxCredentialBus::connect(&format!("unix:path={}", path.display()))
        .await
        .unwrap();
    let mut store = LinuxCredentialStore::connect(bus).await.unwrap();
    let mut application = CredentialApplication::default();
    let request = application.begin(CredentialCandidate::new(
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap(),
        Persistence::Durable,
    ));
    application
        .accept(
            request,
            Ok(ValidatedAccount::new("dummy-runner".to_owned())),
            Timestamp::from_seconds(1),
            &mut store,
        )
        .await
        .unwrap();
    *fault.lock().unwrap() = Fault::Write;
    let request = application.begin_revalidation().unwrap();
    application
        .accept(
            request,
            Ok(ValidatedAccount::new("dummy-runner".to_owned())),
            Timestamp::from_seconds(2),
            &mut store,
        )
        .await
        .unwrap();
    assert_eq!(
        store.load().await.unwrap().unwrap().validated_at(),
        Timestamp::from_seconds(2)
    );
    let request = application.begin_revalidation().unwrap();
    assert_eq!(
        application
            .accept(
                request,
                Err(tallyline_submission::api::model::CredentialFailure::InvalidKey),
                Timestamp::from_seconds(3),
                &mut store
            )
            .await,
        Err(tallyline_submission::api::model::CredentialFailure::InvalidKey)
    );
    assert_eq!(
        CredentialApplication::load(&mut store)
            .await
            .unwrap()
            .state(),
        &tallyline_submission::api::model::AccountState::Rejected(
            tallyline_submission::api::model::CredentialFailure::InvalidKey
        )
    );
    *fault.lock().unwrap() = Fault::CommitWithoutReply;
    let request = application.begin_revalidation().unwrap();
    assert_eq!(
        application
            .accept(
                request,
                Ok(ValidatedAccount::new("dummy-runner".to_owned())),
                Timestamp::from_seconds(4),
                &mut store
            )
            .await,
        Err(
            tallyline_submission::api::model::CredentialFailure::Storage(
                StorageFailure::OutcomeUnknown
            )
        )
    );
    assert_eq!(
        application.state(),
        &tallyline_submission::api::model::AccountState::Rejected(
            tallyline_submission::api::model::CredentialFailure::InvalidKey
        )
    );
    *fault.lock().unwrap() = Fault::None;
    let reconciled = store.load().await.unwrap().unwrap();
    assert_eq!(reconciled.validated_at(), Timestamp::from_seconds(4));
    assert_eq!(reconciled.refusal(), None);
    drop(store);
    server.join().unwrap().unwrap();
}
