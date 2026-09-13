import { Button, Input, InputNumber, Modal } from "antd";
import type { FC } from "react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import {
  deleteSyncPeerSecret,
  setSyncPeerSecret,
  testSyncPeerSecret,
} from "@/commands";
import { updateSettings } from "@/stores/settings";
import type { Settings, SyncPeerSettings } from "@/types/settings";
import { getMessageApi } from "@/utils/feedback";
import ControlFrame from "./ControlFrame";

interface EditablePeer extends SyncPeerSettings {
  secret: string;
}

interface SyncPeersControlProps {
  disabled: boolean;
  settings: Settings;
}

const SyncPeersControl: FC<SyncPeersControlProps> = ({
  disabled,
  settings,
}) => {
  const { t } = useTranslation("preferences");
  const [open, setOpen] = useState(false);
  const [saving, setSaving] = useState(false);
  const [peers, setPeers] = useState<EditablePeer[]>([]);

  const openManager = () => {
    setPeers(settings.sync.peers.map((peer) => ({ ...peer, secret: "" })));
    setOpen(true);
  };

  const addPeer = () => {
    const id = crypto.randomUUID();
    setPeers((current) => [
      ...current,
      {
        address: "",
        id,
        port: 45_321,
        secret: "",
        secretReference: `sync-peer:${id}`,
      },
    ]);
  };

  const updatePeer = <K extends keyof EditablePeer>(
    index: number,
    key: K,
    value: EditablePeer[K],
  ) => {
    setPeers((current) =>
      current.map((peer, peerIndex) =>
        peerIndex === index ? { ...peer, [key]: value } : peer,
      ),
    );
  };

  const save = async () => {
    const originalIds = new Set(settings.sync.peers.map((peer) => peer.id));
    const newPeers = peers.filter((peer) => !originalIds.has(peer.id));
    if (
      peers.some((peer) => !peer.address.trim()) ||
      newPeers.some((peer) => !peer.secret)
    ) {
      getMessageApi().error(t("syncPeers.required"));
      return;
    }

    setSaving(true);
    try {
      for (const peer of peers) {
        if (peer.secret) await setSyncPeerSecret(peer.id, peer.secret);
      }

      const nextPeers = peers.map(({ secret: _secret, ...peer }) => peer);
      await updateSettings({ sync: { peers: nextPeers } });

      const nextIds = new Set(nextPeers.map((peer) => peer.id));
      for (const peer of settings.sync.peers) {
        if (!nextIds.has(peer.id)) await deleteSyncPeerSecret(peer.id);
      }

      getMessageApi().success(t("syncPeers.saved"));
      setOpen(false);
    } finally {
      setSaving(false);
    }
  };

  const testPeer = async (peer: EditablePeer) => {
    if (peer.secret) await setSyncPeerSecret(peer.id, peer.secret);
    await testSyncPeerSecret(peer.id);
    getMessageApi().success(t("syncPeers.keyValid"));
  };

  return (
    <>
      <ControlFrame>
        <Button disabled={disabled} onClick={openManager}>
          {t("schema.settings.sync.peers.controlLabel")}
        </Button>
      </ControlFrame>
      <Modal
        cancelText={t("syncPeers.cancel")}
        confirmLoading={saving}
        okText={t("syncPeers.save")}
        onCancel={() => setOpen(false)}
        onOk={save}
        open={open}
        title={t("syncPeers.title")}
        width={720}
      >
        <div className="flex flex-col gap-3">
          {peers.map((peer, index) => (
            <div
              className="grid grid-cols-[1fr_7rem_1fr_auto_auto] gap-2"
              key={peer.id}
            >
              <Input
                onChange={(event) =>
                  updatePeer(index, "address", event.target.value)
                }
                placeholder={t("syncPeers.address")}
                value={peer.address}
              />
              <InputNumber
                className="w-full"
                max={65_535}
                min={1}
                onChange={(value) => updatePeer(index, "port", value ?? 45_321)}
                value={peer.port}
              />
              <Input.Password
                onChange={(event) =>
                  updatePeer(index, "secret", event.target.value)
                }
                placeholder={t("syncPeers.secret")}
                value={peer.secret}
              />
              <Button onClick={() => testPeer(peer)}>
                {t("syncPeers.testKey")}
              </Button>
              <Button
                danger
                onClick={() =>
                  setPeers((current) => current.filter((_, i) => i !== index))
                }
              >
                {t("syncPeers.remove")}
              </Button>
            </div>
          ))}
          <Button block onClick={addPeer} type="dashed">
            {t("syncPeers.add")}
          </Button>
        </div>
      </Modal>
    </>
  );
};

export default SyncPeersControl;
