//! Locked folders in the library (docs/age-locked-folders.md, D-018): which folders a lock
//! covers, the keys unlocked for this session, what stays hidden, and the lock commands.
//!
//! A lock covers its folder and every folder under it. An item in a covered folder is
//! encrypted with an item key of its own, wrapped once per lock that covers it.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt;
use std::sync::{Arc, MutexGuard, PoisonError};

use library_core::{FOLDER_COLORS, PivPolicy, PivPrompt, PivWrap, RecoveryKey, SourceError};
use rusqlite::{Connection, OptionalExtension, params};

use crate::error::{Context, Result};
use crate::library::Library;
use crate::piv::{self, PivCard};
use crate::time::now_ms;
use crate::vault::{
    FolderKey, ItemKey, KdfParams, RecoverySecret, SealedFields, VaultError, WrapKey, WrapRole,
    new_salt, password,
};

/// The keys unlocked in this session, and what was opened with them. Never written anywhere;
/// dropped (and wiped) when a folder is locked again or the library closes.
#[derive(Default)]
pub(crate) struct Keyring {
    folders: HashMap<String, FolderKey>,
    items: HashMap<String, Arc<ItemKey>>,
    fields: HashMap<String, SealedFields>,
}

impl fmt::Debug for Keyring {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Keyring")
            .field("unlocked", &self.folders.len())
            .finish_non_exhaustive()
    }
}

impl Keyring {
    pub(crate) fn is_unlocked(&self, folder: &str) -> bool {
        self.folders.contains_key(folder)
    }

    pub(crate) fn unlocked(&self) -> impl Iterator<Item = &str> {
        self.folders.keys().map(String::as_str)
    }

    pub(crate) fn folder(&self, folder: &str) -> Option<&FolderKey> {
        self.folders.get(folder)
    }

    pub(crate) fn cached_item(&self, item: &str) -> Option<Arc<ItemKey>> {
        self.items.get(item).cloned()
    }

    pub(crate) fn remember_item(&mut self, item: &str, key: Arc<ItemKey>) {
        self.items.insert(item.to_owned(), key);
    }

    pub(crate) fn cached_fields(&self, item: &str) -> Option<&SealedFields> {
        self.fields.get(item)
    }

    pub(crate) fn remember_fields(&mut self, item: &str, fields: SealedFields) {
        self.fields.insert(item.to_owned(), fields);
    }

    pub(crate) fn forget_item(&mut self, item: &str) {
        self.items.remove(item);
        self.fields.remove(item);
    }

    fn insert(&mut self, folder: &str, key: FolderKey) {
        self.folders.insert(folder.to_owned(), key);
    }

    /// Locks a folder again. Item keys and fields opened through any folder go too: another
    /// unlocked folder opens them again when needed.
    fn forget(&mut self, folder: &str) {
        self.folders.remove(folder);
        self.items.clear();
        self.fields.clear();
    }

    pub(crate) fn clear(&mut self) {
        self.folders.clear();
        self.items.clear();
        self.fields.clear();
    }
}

/// The folder tree as far as locks are concerned.
#[derive(Debug, Clone, Default)]
pub(crate) struct LockMap {
    parent: HashMap<String, Option<String>>,
    names: HashMap<String, String>,
    /// Locked folders, and whether each has a password (an identity). A folder imported locked
    /// from Eagle has none: it is hidden, and nothing is encrypted for it.
    locks: HashMap<String, bool>,
}

impl LockMap {
    pub(crate) fn load(conn: &Connection) -> Result<Self> {
        let mut map = Self::default();
        let mut statement = conn
            .prepare_cached(
                "SELECT id, parent_id, name, locked, wrapped_identity IS NOT NULL FROM folder",
            )
            .context("read locks")?;
        let mut rows = statement.query([]).context("read locks")?;
        while let Some(row) = rows.next().context("read locks")? {
            let id: String = row.get(0).context("read a lock")?;
            map.parent
                .insert(id.clone(), row.get(1).context("read a lock")?);
            map.names
                .insert(id.clone(), row.get(2).context("read a lock")?);
            if row.get::<_, bool>(3).context("read a lock")? {
                map.locks.insert(id, row.get(4).context("read a lock")?);
            }
        }
        Ok(map)
    }

    pub(crate) fn has_locks(&self) -> bool {
        !self.locks.is_empty()
    }

    pub(crate) fn is_lock(&self, folder: &str) -> bool {
        self.locks.contains_key(folder)
    }

    pub(crate) fn name(&self, folder: &str) -> String {
        self.names.get(folder).cloned().unwrap_or_default()
    }

    /// The same tree with `folder` under `parent`: what a move would make.
    pub(crate) fn moved(&self, folder: &str, parent: Option<&str>) -> Self {
        let mut map = self.clone();
        map.parent
            .insert(folder.to_owned(), parent.map(str::to_owned));
        map
    }

    /// The same tree without `folders` and their locks: what a delete would make.
    pub(crate) fn without(&self, folders: &HashSet<String>) -> Self {
        let mut map = self.clone();
        map.locks.retain(|id, _| !folders.contains(id));
        map
    }

    /// The same tree with `folder` no longer locked.
    pub(crate) fn unlocking(&self, folder: &str) -> Self {
        let mut map = self.clone();
        map.locks.remove(folder);
        map
    }

    /// Locks with a password at or above `folder`: the locks that encrypt its items.
    pub(crate) fn sealing<'a>(&'a self, folder: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        let mut at = Some(folder);
        let mut seen = HashSet::new();
        std::iter::from_fn(move || {
            while let Some(id) = at {
                if !seen.insert(id) {
                    return None;
                }
                let current = id;
                at = self.parent.get(id).and_then(|parent| parent.as_deref());
                if self.locks.get(current) == Some(&true) {
                    return Some(current);
                }
            }
            None
        })
    }

    /// Every folder under a lock that is not unlocked, the locked folder included: their items
    /// are hidden in every scope (docs/architecture.md §6.4).
    pub(crate) fn hidden(&self, keys: &Keyring) -> HashSet<String> {
        let mut children: HashMap<&str, Vec<&str>> = HashMap::new();
        for (id, parent) in &self.parent {
            if let Some(parent) = parent {
                children
                    .entry(parent.as_str())
                    .or_default()
                    .push(id.as_str());
            }
        }
        let mut hidden = HashSet::new();
        let mut stack: Vec<&str> = self
            .locks
            .keys()
            .map(String::as_str)
            .filter(|id| !keys.is_unlocked(id))
            .collect();
        while let Some(id) = stack.pop() {
            if hidden.insert(id.to_owned()) {
                stack.extend(children.get(id).into_iter().flatten().copied());
            }
        }
        hidden
    }

    /// `folder` and every folder under it.
    pub(crate) fn subtree(&self, folder: &str) -> HashSet<String> {
        let mut found: HashSet<String> = HashSet::from([folder.to_owned()]);
        loop {
            let before = found.len();
            for (id, parent) in &self.parent {
                if parent.as_ref().is_some_and(|parent| found.contains(parent)) {
                    found.insert(id.clone());
                }
            }
            if found.len() == before {
                return found;
            }
        }
    }
}

/// The SQL that keeps an item visible: in no hidden folder, and, when encrypted, wrapped for a
/// folder unlocked in this session. Binds `:hidden` and `:unlocked` (JSON arrays).
pub(crate) const VISIBLE: &str = "NOT EXISTS (SELECT 1 FROM item_folder \
     WHERE item_folder.item_id = item.id \
     AND item_folder.folder_id IN (SELECT value FROM json_each(:hidden))) \
     AND (item.encrypted = 0 OR EXISTS (SELECT 1 FROM item_key_wrap \
     WHERE item_key_wrap.item_id = item.id \
     AND item_key_wrap.folder_id IN (SELECT value FROM json_each(:unlocked))))";

/// What [`VISIBLE`] binds.
pub(crate) struct Visibility {
    pub hidden: String,
    pub unlocked: String,
    pub hidden_folders: HashSet<String>,
}

impl Library {
    pub(crate) fn keys(&self) -> MutexGuard<'_, Keyring> {
        self.keyring()
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Who may see what right now. Takes the connection the caller holds.
    pub(crate) fn visibility(&self, conn: &Connection) -> Result<Visibility> {
        let map = LockMap::load(conn)?;
        let keys = self.keys();
        let hidden_folders = map.hidden(&keys);
        let unlocked: Vec<&str> = keys.unlocked().collect();
        Ok(Visibility {
            hidden: serde_json::to_string(&hidden_folders).unwrap_or_default(),
            unlocked: serde_json::to_string(&unlocked).unwrap_or_default(),
            hidden_folders,
        })
    }

    /// Puts a lock on `id`: a new identity wrapped by the password and by a recovery secret,
    /// then every item the folder covers is encrypted (`progress(done, total)`). The folder is
    /// locked when this returns; the secret is shown once and never stored.
    pub fn lock_folder(
        &self,
        id: &str,
        new_password: String,
        label: Option<String>,
        label_color: Option<String>,
        progress: &dyn Fn(u32, u32),
    ) -> Result<RecoveryKey> {
        self.check_writable()?;
        let new_password = password(new_password);
        check_password(&new_password)?;
        let (label, label_color) = check_label(label, label_color)?;
        let map = LockMap::load(&self.conn())?;
        if !map.names.contains_key(id) {
            return Err(SourceError::NotFound);
        }
        if map.is_lock(id) {
            return Err(SourceError::Invalid(
                "This folder is already locked.".into(),
            ));
        }
        // Items the new lock covers that other locks encrypt need one of those keys.
        let key = FolderKey::generate();
        let covered = self.items_in(&map.subtree(id))?;
        let mut planned = map.clone();
        planned.locks.insert(id.to_owned(), true);
        self.check_reseal(&covered, &planned, &|_, folders| folders, Some((id, &key)))?;

        let salt = new_salt();
        let params = new_kdf_params();
        let password_wrap = key.wrap(
            &WrapKey::from_password(&new_password, &salt, params).map_err(vault_error)?,
            WrapRole::Password,
            id,
        );
        let secret = RecoverySecret::generate();
        let recovery_wrap = key.wrap(&WrapKey::from_recovery(&secret), WrapRole::Recovery, id);

        let now = now_ms();
        {
            let mut conn = self.conn();
            let tx = conn.transaction().context("lock the folder")?;
            tx.execute(
                "UPDATE folder SET locked = 1, label = ?2, label_color = ?3, kdf_salt = ?4,
                     kdf_params = ?5, wrapped_identity = ?6, recovery_wrap = ?7, updated_at = ?8
                 WHERE id = ?1",
                params![
                    id,
                    label,
                    label_color,
                    salt.as_slice(),
                    params.to_json(),
                    password_wrap,
                    recovery_wrap,
                    now
                ],
            )
            .context("lock the folder")?;
            log_folder(&tx, id, now)?;
            tx.commit().context("lock the folder")?;
        }
        self.keys().insert(id, key);
        // The folder is locked at once; if encrypting stops here, the next unlock finishes it.
        let sealed = self.reseal(&covered, progress);
        self.keys().forget(id);
        sealed?;
        Ok(RecoveryKey {
            key: secret.key_text(),
            phrase: secret.phrase(),
        })
    }

    /// Opens a locked folder for this session with its password. Encryption that an earlier
    /// lock or a crash left unfinished is completed.
    pub fn unlock_folder(&self, id: &str, given: String) -> Result<()> {
        let key = self.open_with_password(id, &password(given))?;
        self.keys().insert(id, key);
        self.repair(id);
        Ok(())
    }

    /// Opens a locked folder with its recovery key or phrase and sets a new password: the same
    /// identity, wrapped again; the files stay as they are. The old password stops working.
    pub fn recover_folder(&self, id: &str, recovery: &str, new_password: String) -> Result<()> {
        self.check_writable()?;
        let new_password = password(new_password);
        check_password(&new_password)?;
        let secret = RecoverySecret::parse(recovery).map_err(vault_error)?;
        let row = self.lock_row(id)?;
        let wrap = row
            .recovery_wrap
            .ok_or_else(|| SourceError::NotSupported("This folder has no recovery key.".into()))?;
        let key = FolderKey::unwrap(
            &wrap,
            &WrapKey::from_recovery(&secret),
            WrapRole::Recovery,
            id,
        )
        .map_err(vault_error)?;
        self.store_password_wrap(id, &key, &new_password)?;
        self.keys().insert(id, key);
        self.repair(id);
        Ok(())
    }

    /// Locks an unlocked folder again: its key leaves memory.
    pub fn relock_folder(&self, id: &str) {
        self.keys().forget(id);
    }

    /// Locks every folder again (closing the library).
    pub fn relock_all(&self) {
        self.keys().clear();
    }

    pub fn is_unlocked(&self, id: &str) -> bool {
        self.keys().is_unlocked(id)
    }

    /// A new password for the same identity; the recovery key stays valid.
    pub fn change_folder_password(
        &self,
        id: &str,
        current: String,
        new_password: String,
    ) -> Result<()> {
        self.check_writable()?;
        let new_password = password(new_password);
        check_password(&new_password)?;
        let key = self.open_with_password(id, &password(current))?;
        self.store_password_wrap(id, &key, &new_password)
    }

    /// A new recovery secret: the old one stops working. Shown once.
    pub fn renew_recovery_key(&self, id: &str, current: String) -> Result<RecoveryKey> {
        self.check_writable()?;
        let key = self.open_with_password(id, &password(current))?;
        let secret = RecoverySecret::generate();
        let wrap = key.wrap(&WrapKey::from_recovery(&secret), WrapRole::Recovery, id);
        let now = now_ms();
        let mut conn = self.conn();
        let tx = conn.transaction().context("renew the recovery key")?;
        tx.execute(
            "UPDATE folder SET recovery_wrap = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, wrap, now],
        )
        .context("renew the recovery key")?;
        log_folder(&tx, id, now)?;
        tx.commit().context("renew the recovery key")?;
        Ok(RecoveryKey {
            key: secret.key_text(),
            phrase: secret.phrase(),
        })
    }

    /// Takes the lock off: the folder's items are decrypted (unless another lock covers them),
    /// then the identity and both wraps are deleted. Until the end the lock stays, so an
    /// interrupted removal loses nothing.
    pub fn remove_folder_lock(
        &self,
        id: &str,
        given: String,
        progress: &dyn Fn(u32, u32),
    ) -> Result<()> {
        self.check_writable()?;
        let key = self.open_with_password(id, &password(given))?;
        self.keys().insert(id, key);
        let map = LockMap::load(&self.conn())?;
        let mut affected = self.items_in(&map.subtree(id))?;
        affected.extend(self.items_wrapped_for(id)?);
        affected.sort();
        affected.dedup();
        let planned = map.unlocking(id);
        self.check_reseal(&affected, &planned, &|_, folders| folders, None)?;
        self.reseal_with(&affected, &planned, &|_, folders| folders, progress)?;
        let now = now_ms();
        let mut conn = self.conn();
        let tx = conn.transaction().context("remove the lock")?;
        tx.execute(
            "DELETE FROM item_key_wrap WHERE folder_id = ?1",
            params![id],
        )
        .context("remove the lock")?;
        tx.execute(
            "DELETE FROM folder_piv_wrap WHERE folder_id = ?1",
            params![id],
        )
        .context("remove the lock")?;
        tx.execute(
            "UPDATE folder SET locked = 0, label = NULL, label_color = NULL, kdf_salt = NULL,
                 kdf_params = NULL, wrapped_identity = NULL, recovery_wrap = NULL, updated_at = ?2
             WHERE id = ?1",
            params![id, now],
        )
        .context("remove the lock")?;
        log_folder(&tx, id, now)?;
        tx.commit().context("remove the lock")?;
        drop(conn);
        self.keys().forget(id);
        Ok(())
    }

    /// The lock's tag: a caption and a colour, plaintext; works while the folder is locked.
    pub fn set_lock_label(
        &self,
        id: &str,
        label: Option<String>,
        label_color: Option<String>,
    ) -> Result<()> {
        self.check_writable()?;
        let (label, label_color) = check_label(label, label_color)?;
        let now = now_ms();
        let mut conn = self.conn();
        let tx = conn.transaction().context("label the lock")?;
        let changed = tx
            .execute(
                "UPDATE folder SET label = ?2, label_color = ?3, updated_at = ?4
                 WHERE id = ?1 AND locked = 1",
                params![id, label, label_color, now],
            )
            .context("label the lock")?;
        if changed == 0 {
            return Err(SourceError::NotFound);
        }
        log_folder(&tx, id, now)?;
        tx.commit().context("label the lock")
    }

    /// YubiKeys bound to this locked folder: serial, retired slot and touch-or-PIN policy.
    /// No recipient and no wrapped identity cross this boundary.
    pub fn piv_wraps(&self, id: &str) -> Result<Vec<PivWrap>> {
        let _ = self.lock_row(id)?;
        let conn = self.conn();
        let mut statement = conn
            .prepare_cached(
                "SELECT card_serial, slot, policy FROM folder_piv_wrap
                 WHERE folder_id = ?1 ORDER BY card_serial",
            )
            .context("read YubiKey wraps")?;
        statement
            .query_map(params![id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .context("read YubiKey wraps")?
            .map(|row| {
                let (serial, slot, policy) = row.context("read a YubiKey wrap")?;
                let slot = u8::try_from(slot).map_err(|_| {
                    SourceError::Storage("unreadable lock data: YubiKey slot".into())
                })?;
                let policy = match policy.as_str() {
                    "touch" => PivPolicy::Touch,
                    "pin" => PivPolicy::Pin,
                    _ => {
                        return Err(SourceError::Storage(
                            "unreadable lock data: YubiKey policy".into(),
                        ));
                    }
                };
                Ok(PivWrap {
                    serial,
                    slot,
                    policy,
                })
            })
            .collect()
    }

    /// Adds one card as a third wrap of this folder's identity. The password opens the
    /// identity; the card's public key wraps it again. Other folders are untouched, and
    /// this folder stays locked.
    pub fn bind_piv(&self, id: &str, given: String, card: &PivCard) -> Result<()> {
        self.check_writable()?;
        let key = self.open_with_password(id, &password(given))?;
        let sealed = piv::seal_identity(&key, &card.recipient).map_err(vault_error)?;
        let now = now_ms();
        let policy = match card.policy {
            PivPolicy::Touch => "touch",
            PivPolicy::Pin => "pin",
        };
        let mut conn = self.conn();
        let tx = conn.transaction().context("add a YubiKey")?;
        let already = tx
            .query_row(
                "SELECT 1 FROM folder_piv_wrap WHERE folder_id = ?1 AND card_serial = ?2",
                params![id, card.serial],
                |_| Ok(()),
            )
            .optional()
            .context("add a YubiKey")?
            .is_some();
        if already {
            return Err(SourceError::Invalid(
                "This YubiKey is already added to the folder.".into(),
            ));
        }
        tx.execute(
            "INSERT INTO folder_piv_wrap
                 (folder_id, card_serial, slot, policy, recipient, wrapped_identity, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                id,
                card.serial,
                i64::from(card.slot),
                policy,
                card.recipient,
                sealed,
                now
            ],
        )
        .context("add a YubiKey")?;
        log_folder(&tx, id, now)?;
        tx.commit().context("add a YubiKey")
    }

    /// Drops one card's wrap. The password still opens the folder; the recovery wrap stays.
    pub fn unbind_piv(&self, id: &str, given: String, serial: &str) -> Result<()> {
        self.check_writable()?;
        let _key = self.open_with_password(id, &password(given))?;
        let now = now_ms();
        let mut conn = self.conn();
        let tx = conn.transaction().context("remove a YubiKey")?;
        let changed = tx
            .execute(
                "DELETE FROM folder_piv_wrap WHERE folder_id = ?1 AND card_serial = ?2",
                params![id, serial],
            )
            .context("remove a YubiKey")?;
        if changed == 0 {
            return Err(SourceError::NotFound);
        }
        log_folder(&tx, id, now)?;
        tx.commit().context("remove a YubiKey")
    }

    /// Opens this folder only, with the named card. The prompt must be the card's policy:
    /// a touch, or the PIN instead of a touch. `agree` performs the P-256 ECDH
    /// for the wrap's retired slot.
    pub fn unlock_with_piv(
        &self,
        id: &str,
        serial: &str,
        prompt: &PivPrompt,
        agree: impl Fn(u8, &[u8]) -> std::result::Result<[u8; 32], SourceError>,
    ) -> Result<()> {
        let row = self.piv_row(id, serial)?;
        if !piv::prompt_matches(row.policy, prompt) {
            return Err(SourceError::Invalid(
                "Touch the YubiKey, or enter its PIN. Not both.".into(),
            ));
        }
        if let PivPrompt::Pin { pin } = prompt {
            piv::check_piv_pin(pin)?;
        }
        let captured = std::cell::RefCell::new(None);
        let slot = row.slot;
        let key = piv::open_identity(&row.wrapped, &row.recipient, |ephemeral| {
            agree(slot, ephemeral).map_err(|error| {
                *captured.borrow_mut() = Some(error);
                VaultError::WrongKey
            })
        });
        let key = match key {
            Ok(key) => key,
            Err(error) => {
                return Err(captured
                    .borrow_mut()
                    .take()
                    .unwrap_or_else(|| vault_error(error)));
            }
        };
        self.keys().insert(id, key);
        self.repair(id);
        Ok(())
    }

    fn piv_row(&self, id: &str, serial: &str) -> Result<PivRow> {
        let _ = self.lock_row(id)?;
        self.conn()
            .query_row(
                "SELECT slot, policy, recipient, wrapped_identity FROM folder_piv_wrap
                 WHERE folder_id = ?1 AND card_serial = ?2",
                params![id, serial],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Vec<u8>>(3)?,
                    ))
                },
            )
            .optional()
            .context("read a YubiKey wrap")?
            .and_then(|(slot, policy, recipient, wrapped)| {
                let policy = match policy.as_str() {
                    "touch" => PivPolicy::Touch,
                    "pin" => PivPolicy::Pin,
                    _ => return None,
                };
                let slot = u8::try_from(slot).ok()?;
                Some(PivRow {
                    slot,
                    policy,
                    recipient,
                    wrapped,
                })
            })
            .ok_or(SourceError::NotFound)
    }

    /// Argon2id of the password against the folder's own salt and parameters.
    fn open_with_password(
        &self,
        id: &str,
        given: &age::secrecy::SecretString,
    ) -> Result<FolderKey> {
        let row = self.lock_row(id)?;
        let (Some(salt), Some(params), Some(wrap)) = (row.salt, row.params, row.password_wrap)
        else {
            return Err(SourceError::NotSupported(
                "This folder has no password yet.".into(),
            ));
        };
        let params = KdfParams::from_json(&params).map_err(vault_error)?;
        let wrap_key = WrapKey::from_password(given, &salt, params).map_err(vault_error)?;
        FolderKey::unwrap(&wrap, &wrap_key, WrapRole::Password, id).map_err(vault_error)
    }

    fn store_password_wrap(
        &self,
        id: &str,
        key: &FolderKey,
        new_password: &age::secrecy::SecretString,
    ) -> Result<()> {
        let salt = new_salt();
        let params = new_kdf_params();
        let wrap = key.wrap(
            &WrapKey::from_password(new_password, &salt, params).map_err(vault_error)?,
            WrapRole::Password,
            id,
        );
        let now = now_ms();
        let mut conn = self.conn();
        let tx = conn.transaction().context("change the password")?;
        tx.execute(
            "UPDATE folder SET kdf_salt = ?2, kdf_params = ?3, wrapped_identity = ?4,
                 updated_at = ?5
             WHERE id = ?1",
            params![id, salt.as_slice(), params.to_json(), wrap, now],
        )
        .context("change the password")?;
        log_folder(&tx, id, now)?;
        tx.commit().context("change the password")
    }

    fn lock_row(&self, id: &str) -> Result<LockRow> {
        self.conn()
            .query_row(
                "SELECT locked, kdf_salt, kdf_params, wrapped_identity, recovery_wrap
                 FROM folder WHERE id = ?1",
                params![id],
                |row| {
                    Ok(LockRow {
                        locked: row.get(0)?,
                        salt: row.get(1)?,
                        params: row.get(2)?,
                        password_wrap: row.get(3)?,
                        recovery_wrap: row.get(4)?,
                    })
                },
            )
            .optional()
            .context("read the lock")?
            .filter(|row| row.locked)
            .ok_or(SourceError::NotFound)
    }

    /// Finishes what an interrupted lock, removal or write left behind for this lock. Only a
    /// writer does it, and a failure is logged: the folder is open either way.
    fn repair(&self, id: &str) {
        if self.check_writable().is_err() {
            return;
        }
        let result = (|| {
            let map = LockMap::load(&self.conn())?;
            let mut items = self.items_in(&map.subtree(id))?;
            items.extend(self.items_wrapped_for(id)?);
            items.sort();
            items.dedup();
            self.reseal(&items, &|_, _| {})
        })();
        if let Err(error) = result {
            tracing::warn!("could not finish encrypting a locked folder: {error}");
        }
    }

    /// Items with a link to any of `folders`, the trash included.
    pub(crate) fn items_in(&self, folders: &HashSet<String>) -> Result<Vec<String>> {
        let json = serde_json::to_string(folders).unwrap_or_default();
        let conn = self.conn();
        let mut statement = conn
            .prepare_cached(
                "SELECT DISTINCT item_id FROM item_folder
                 WHERE folder_id IN (SELECT value FROM json_each(?1)) ORDER BY item_id",
            )
            .context("find covered items")?;
        statement
            .query_map(params![json], |row| row.get(0))
            .and_then(Iterator::collect)
            .context("find covered items")
    }

    fn items_wrapped_for(&self, folder: &str) -> Result<Vec<String>> {
        let conn = self.conn();
        let mut statement = conn
            .prepare_cached("SELECT item_id FROM item_key_wrap WHERE folder_id = ?1")
            .context("find wrapped items")?;
        statement
            .query_map(params![folder], |row| row.get(0))
            .and_then(Iterator::collect)
            .context("find wrapped items")
    }
}

struct PivRow {
    slot: u8,
    policy: PivPolicy,
    recipient: String,
    wrapped: Vec<u8>,
}

struct LockRow {
    locked: bool,
    salt: Option<Vec<u8>>,
    params: Option<String>,
    password_wrap: Option<Vec<u8>>,
    recovery_wrap: Option<Vec<u8>>,
}

/// Locks the app makes use the default parameters; tests use cheap ones.
fn new_kdf_params() -> KdfParams {
    #[cfg(test)]
    {
        KdfParams::FAST
    }
    #[cfg(not(test))]
    {
        KdfParams::DEFAULT
    }
}

fn check_password(given: &age::secrecy::SecretString) -> Result<()> {
    use age::secrecy::ExposeSecret;
    if given.expose_secret().is_empty() {
        return Err(SourceError::Invalid("Enter a password.".into()));
    }
    Ok(())
}

/// The lock's caption (trimmed, at most 60 characters) and one of the folder colours.
fn check_label(
    label: Option<String>,
    color: Option<String>,
) -> Result<(Option<String>, Option<String>)> {
    let label = label
        .map(|label| label.trim().to_owned())
        .filter(|label| !label.is_empty());
    if label
        .as_ref()
        .is_some_and(|label| label.chars().count() > 60)
    {
        return Err(SourceError::Invalid(
            "The lock's label is longer than 60 characters.".into(),
        ));
    }
    if let Some(color) = &color
        && !FOLDER_COLORS.contains(&color.as_str())
    {
        return Err(SourceError::Invalid(format!("Unknown colour: {color}.")));
    }
    Ok((label, color))
}

pub(crate) fn vault_error(error: VaultError) -> SourceError {
    match error {
        VaultError::WrongKey | VaultError::BadRecoveryText => SourceError::WrongPassword,
        VaultError::Corrupt(what) => SourceError::Storage(format!("unreadable lock data: {what}")),
        VaultError::Age(message) => SourceError::Storage(format!("decrypt: {message}")),
    }
}

fn log_folder(tx: &rusqlite::Transaction<'_>, id: &str, now: i64) -> Result<()> {
    tx.execute(
        "INSERT INTO change_log (entity, entity_id, op, at) VALUES ('folder', ?1, 'update', ?2)",
        params![id, now],
    )
    .context("log the change")?;
    Ok(())
}

/// The locks with a password that cover an item with these folder links.
pub(crate) fn sealing_locks(map: &LockMap, folders: &[String]) -> BTreeSet<String> {
    folders
        .iter()
        .flat_map(|folder| map.sealing(folder))
        .map(str::to_owned)
        .collect()
}
