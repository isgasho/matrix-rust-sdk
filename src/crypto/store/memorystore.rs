// Copyright 2020 The Matrix.org Foundation C.I.C.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;

use super::{Account, CryptoStore, InboundGroupSession, Result, Session};
use crate::crypto::device::Device;
use crate::crypto::memory_stores::{DeviceStore, GroupSessionStore, SessionStore, UserDevices};
use crate::identifiers::{DeviceId, RoomId, UserId};

#[derive(Debug)]
pub struct MemoryStore {
    sessions: SessionStore,
    inbound_group_sessions: GroupSessionStore,
    tracked_users: HashSet<UserId>,
    devices: DeviceStore,
}

impl MemoryStore {
    pub fn new() -> Self {
        MemoryStore {
            sessions: SessionStore::new(),
            inbound_group_sessions: GroupSessionStore::new(),
            tracked_users: HashSet::new(),
            devices: DeviceStore::new(),
        }
    }
}

#[async_trait]
impl CryptoStore for MemoryStore {
    async fn load_account(&mut self) -> Result<Option<Account>> {
        Ok(None)
    }

    async fn save_account(&mut self, _: Account) -> Result<()> {
        Ok(())
    }

    async fn save_session(&mut self, session: Session) -> Result<()> {
        self.sessions.add(session).await;
        Ok(())
    }

    async fn get_sessions(&mut self, sender_key: &str) -> Result<Option<Arc<Mutex<Vec<Session>>>>> {
        Ok(self.sessions.get(sender_key))
    }

    async fn save_inbound_group_session(&mut self, session: InboundGroupSession) -> Result<bool> {
        Ok(self.inbound_group_sessions.add(session))
    }

    async fn get_inbound_group_session(
        &mut self,
        room_id: &RoomId,
        sender_key: &str,
        session_id: &str,
    ) -> Result<Option<InboundGroupSession>> {
        Ok(self
            .inbound_group_sessions
            .get(room_id, sender_key, session_id))
    }

    fn tracked_users(&self) -> &HashSet<UserId> {
        &self.tracked_users
    }

    async fn add_user_for_tracking(&mut self, user: &UserId) -> Result<bool> {
        Ok(self.tracked_users.insert(user.clone()))
    }

    async fn get_device(&self, user_id: &UserId, device_id: &DeviceId) -> Result<Option<Device>> {
        Ok(self.devices.get(user_id, device_id))
    }

    async fn get_user_devices(&self, user_id: &UserId) -> Result<UserDevices> {
        Ok(self.devices.user_devices(user_id))
    }

    async fn save_device(&self, device: Device) -> Result<()> {
        self.devices.add(device);
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::convert::TryFrom;

    use crate::crypto::device::test::get_device;
    use crate::crypto::olm::test::get_account_and_session;
    use crate::crypto::olm::{InboundGroupSession, OutboundGroupSession};
    use crate::crypto::store::memorystore::MemoryStore;
    use crate::crypto::store::CryptoStore;
    use crate::identifiers::RoomId;

    #[tokio::test]
    async fn test_session_store() {
        let (account, session) = get_account_and_session().await;
        let mut store = MemoryStore::new();

        assert!(store.load_account().await.unwrap().is_none());
        store.save_account(account).await.unwrap();

        store.save_session(session.clone()).await.unwrap();

        let sessions = store
            .get_sessions(&session.sender_key)
            .await
            .unwrap()
            .unwrap();
        let sessions = sessions.lock().await;

        let loaded_session = &sessions[0];

        assert_eq!(&session, loaded_session);
    }

    #[tokio::test]
    async fn test_group_session_store() {
        let room_id = RoomId::try_from("!test:localhost").unwrap();

        let outbound = OutboundGroupSession::new(&room_id);
        let inbound = InboundGroupSession::new(
            "test_key",
            "test_key",
            &room_id,
            outbound.session_key().await,
        )
        .unwrap();

        let mut store = MemoryStore::new();
        store
            .save_inbound_group_session(inbound.clone())
            .await
            .unwrap();

        let loaded_session = store
            .get_inbound_group_session(&room_id, "test_key", outbound.session_id())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(inbound, loaded_session);
    }

    #[tokio::test]
    async fn test_device_store() {
        let device = get_device();
        let store = MemoryStore::new();

        store.save_device(device.clone()).await.unwrap();

        let loaded_device = store
            .get_device(device.user_id(), device.device_id())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(device, loaded_device);

        let user_devices = store.get_user_devices(device.user_id()).await.unwrap();

        assert_eq!(user_devices.keys().nth(0).unwrap(), device.device_id());
        assert_eq!(user_devices.devices().nth(0).unwrap(), &device);

        let loaded_device = user_devices.get(device.device_id()).unwrap();

        assert_eq!(device, loaded_device);
    }

    #[tokio::test]
    async fn test_tracked_users() {
        let device = get_device();
        let mut store = MemoryStore::new();

        assert!(store.add_user_for_tracking(device.user_id()).await.unwrap());
        assert!(!store.add_user_for_tracking(device.user_id()).await.unwrap());

        let tracked_users = store.tracked_users();

        tracked_users.contains(device.user_id());
    }
}
