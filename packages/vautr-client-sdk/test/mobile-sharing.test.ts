import { describe, expect, it, vi } from 'vitest';
import { MobileSharingClient, type SharingRelay, type VautrNativeBridge } from '../src/mobile';

/** A fake native bridge that records calls and returns canned crypto results. */
function fakeNative(overrides: Partial<VautrNativeBridge> = {}): VautrNativeBridge {
  return {
    initialize: vi.fn(),
    unlock: vi.fn(),
    listOverviews: vi.fn(),
    revealSecret: vi.fn(),
    releaseSecret: vi.fn(),
    renderInOverlay: vi.fn(),
    lock: vi.fn(),
    sync: vi.fn(),
    setSecureEnclaveBridge: vi.fn(),
    ensureSharingKey: vi.fn().mockResolvedValue('cHVi'),
    setSharingSecret: vi.fn(),
    shareItem: vi.fn().mockResolvedValue(
      JSON.stringify({
        item_uuid: 'item-1',
        recipient_uuid: 'rec-1',
        wrapped_sik: 'wk',
        ephemeral_public_key: 'epk',
        encrypted_payload: 'ct',
      }),
    ),
    acceptShare: vi.fn().mockResolvedValue(new Uint8Array([1, 2, 3])),
    createGroup: vi.fn(),
    addGroupMember: vi.fn(),
    unwrapGroupKey: vi.fn(),
    encryptGroupItem: vi.fn(),
    decryptGroupItem: vi.fn(),
    ...overrides,
  } as unknown as VautrNativeBridge;
}

/** A fake relay (subset of MobileApiClient) recording the HTTP calls. */
function fakeRelay(): SharingRelay {
  return {
    publishSharingPublicKey: vi.fn(),
    getSharingPublicKey: vi.fn().mockResolvedValue('rcpt-pub'),
    createShare: vi.fn(),
    uploadSharePayload: vi.fn(),
    listShareInbox: vi.fn().mockResolvedValue([
      {
        share_id: 's1',
        sender_uuid: 'snd-1',
        item_uuid: 'item-9',
        wrapped_sik: 'w',
        ephemeral_public_key: 'e',
        encrypted_payload: 'p',
      },
    ]),
    createGroup: vi.fn(),
    addGroupMember: vi.fn(),
    listGroupItems: vi.fn(),
    addGroupItem: vi.fn(),
    groupInbox: vi.fn().mockResolvedValue([]),
  };
}

describe('MobileSharingClient (native-gated sharing orchestration)', () => {
  it('shareItem fetches the recipient key, encrypts natively, relays the bundle', async () => {
    const native = fakeNative();
    const relay = fakeRelay();
    const client = new MobileSharingClient(native, relay);

    await client.shareItem('snd-1', 'rec-1', 'item-1', new Uint8Array([9, 9]));

    expect(relay.getSharingPublicKey).toHaveBeenCalledWith('rec-1');
    expect(native.shareItem).toHaveBeenCalledWith(
      'snd-1',
      'rec-1',
      'item-1',
      'rcpt-pub',
      new Uint8Array([9, 9]),
    );
    expect(relay.createShare).toHaveBeenCalledWith({
      item_uuid: 'item-1',
      recipient_uuid: 'rec-1',
      wrapped_sik: 'wk',
      ephemeral_public_key: 'epk',
    });
    expect(relay.uploadSharePayload).toHaveBeenCalledWith('item-1', 'ct');
  });

  it('collectShares decrypts each inbox entry via the native bridge', async () => {
    const native = fakeNative();
    const relay = fakeRelay();
    const client = new MobileSharingClient(native, relay);

    const shares = await client.collectShares();

    expect(native.acceptShare).toHaveBeenCalledTimes(1);
    expect(shares).toHaveLength(1);
    expect(shares[0].itemUuid).toBe('item-9');
    expect(Array.from(shares[0].plaintext)).toEqual([1, 2, 3]);
  });
});
