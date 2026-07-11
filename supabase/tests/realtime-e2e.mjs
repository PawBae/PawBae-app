import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';

const url = process.env.SUPABASE_URL;
const publishableKey = process.env.SUPABASE_PUBLISHABLE_KEY;

if (!url || !publishableKey) {
  throw new Error('SUPABASE_URL and SUPABASE_PUBLISHABLE_KEY are required');
}

async function request(path, { token, method = 'GET', body, allowFailure = false } = {}) {
  const response = await fetch(`${url}${path}`, {
    method,
    headers: {
      apikey: publishableKey,
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
      ...(body ? { 'Content-Type': 'application/json', Prefer: 'return=representation' } : {}),
    },
    body: body ? JSON.stringify(body) : undefined,
  });
  const text = await response.text();
  const data = text ? JSON.parse(text) : null;
  if (!allowFailure && !response.ok) {
    throw new Error(`${method} ${path} failed (${response.status})`);
  }
  return { data, response };
}

async function signUp(label) {
  const suffix = randomUUID().replaceAll('-', '').slice(0, 12);
  const { data } = await request('/auth/v1/signup', {
    method: 'POST',
    body: {
      email: `realtime-${label}-${suffix}@example.test`,
      password: `T3st-${randomUUID()}`,
      data: { user_name: `spoof-${label}` },
    },
  });
  assert.equal(typeof data.access_token, 'string');
  assert.equal(typeof data.user?.id, 'string');
  return Object.freeze({ id: data.user.id, token: data.access_token });
}

async function rpc(name, body, token) {
  const { data } = await request(`/rest/v1/rpc/${name}`, { body, method: 'POST', token });
  return Array.isArray(data) ? data[0] : data;
}

function realtimeUrl() {
  const socketUrl = new URL('/realtime/v1/websocket', url);
  socketUrl.protocol = socketUrl.protocol === 'https:' ? 'wss:' : 'ws:';
  socketUrl.searchParams.set('apikey', publishableKey);
  socketUrl.searchParams.set('vsn', '1.0.0');
  return socketUrl;
}

async function openPrivateChannel(topic, token, { allowDenied = false } = {}) {
  const socket = new WebSocket(realtimeUrl());
  let state = Object.freeze({ messages: [], waiters: [] });

  const dispatch = (message) => {
    const waiter = state.waiters.find(({ predicate }) => predicate(message));
    if (waiter) {
      clearTimeout(waiter.timeoutId);
      state = Object.freeze({
        ...state,
        waiters: state.waiters.filter(({ id }) => id !== waiter.id),
      });
      waiter.resolve(message);
      return;
    }
    state = Object.freeze({ ...state, messages: [...state.messages, message] });
  };

  socket.addEventListener('message', ({ data }) => dispatch(JSON.parse(String(data))));
  socket.addEventListener('close', ({ code, reason }) =>
    dispatch({ event: 'transport_close', payload: { status: code, response: { reason } } }),
  );
  socket.addEventListener('error', () =>
    dispatch({ event: 'transport_error', payload: { status: 'error' } }),
  );

  const waitFor = (predicate, timeoutMs = 5_000) => {
    const messageIndex = state.messages.findIndex(predicate);
    if (messageIndex >= 0) {
      const message = state.messages[messageIndex];
      state = Object.freeze({
        ...state,
        messages: state.messages.filter((_, index) => index !== messageIndex),
      });
      return Promise.resolve(message);
    }

    return new Promise((resolve, reject) => {
      const id = randomUUID();
      const timeoutId = setTimeout(() => {
        const summaries = state.messages.map((message) => ({
          event: message.event,
          topic: message.topic,
          ref: message.ref,
          joinRef: message.join_ref,
          status: message.payload?.status,
          reason: message.payload?.response?.reason,
        }));
        state = Object.freeze({
          ...state,
          waiters: state.waiters.filter((waiter) => waiter.id !== id),
        });
        reject(new Error(`realtime message timeout; envelopes=${JSON.stringify(summaries)}`));
      }, timeoutMs);
      state = Object.freeze({
        ...state,
        waiters: [...state.waiters, Object.freeze({ id, predicate, resolve, timeoutId })],
      });
    });
  };

  await new Promise((resolve, reject) => {
    const timeoutId = setTimeout(() => reject(new Error('realtime connection timeout')), 5_000);
    socket.addEventListener(
      'open',
      () => {
        clearTimeout(timeoutId);
        resolve();
      },
      { once: true },
    );
    socket.addEventListener(
      'error',
      () => {
        clearTimeout(timeoutId);
        reject(new Error('realtime connection failed'));
      },
      { once: true },
    );
  });

  const ref = randomUUID();
  socket.send(
    JSON.stringify({
      topic: `realtime:${topic}`,
      event: 'phx_join',
      payload: {
        config: {
          broadcast: { ack: false, self: false },
          presence: { key: '', enabled: false },
          postgres_changes: [],
          private: true,
        },
        access_token: token,
      },
      ref,
      join_ref: ref,
    }),
  );
  let status;
  try {
    const reply = await waitFor(
      (message) => message.event === 'phx_reply' && message.ref === ref,
    );
    status = reply.payload?.status;
  } catch (error) {
    const isDeniedTimeout =
      allowDenied &&
      error instanceof Error &&
      error.message.startsWith('realtime message timeout');
    if (!isDeniedTimeout) {
      socket.close();
      throw error;
    }
    status = 'error';
  }

  return Object.freeze({
    close: () => socket.close(),
    status,
    waitForBroadcast: (event, timeoutMs) =>
      waitFor(
        (message) => message.event === 'broadcast' && message.payload?.event === event,
        timeoutMs,
      ),
  });
}

async function expectNoBroadcast(channel, event, timeoutMs = 1_250) {
  try {
    await channel.waitForBroadcast(event, timeoutMs);
  } catch (error) {
    if (error instanceof Error && error.message.startsWith('realtime message timeout')) return;
    throw error;
  }
  assert.fail(`unexpected ${event} broadcast after authorization was revoked`);
}

const visitor = await signUp('visitor');
const host = await signUp('host');
const stranger = await signUp('stranger');

await rpc('send_friend_request', { p_target_user_id: host.id }, visitor.token);
await rpc('accept_friend_request', { p_requester_user_id: visitor.id }, host.token);
const visit = await rpc(
  'request_visit',
  { p_host_user_id: host.id, p_idempotency_key: randomUUID() },
  visitor.token,
);
await rpc(
  'accept_visit',
  { p_visit_id: visit.id, p_idempotency_key: randomUUID() },
  host.token,
);

const topic = `pet:${visitor.id}:${visit.id}`;
const hostChannel = await openPrivateChannel(topic, host.token);
assert.equal(hostChannel.status, 'ok');

const strangerChannel = await openPrivateChannel(topic, stranger.token, { allowDenied: true });
assert.equal(strangerChannel.status, 'error');
strangerChannel.close();

await rpc(
  'update_projection',
  { p_pet_id: 'yoonie', p_skin_id: 'yoonie', p_status: 'working' },
  visitor.token,
);
const projectionMessage = await hostChannel.waitForBroadcast('projection_updated');
assert.deepEqual(Object.keys(projectionMessage.payload.payload).toSorted(), [
  'displayName',
  'id',
  'petId',
  'skinId',
  'status',
  'updatedAt',
  'v',
]);
assert.match(
  projectionMessage.payload.payload.id,
  /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/,
);
assert.equal(projectionMessage.payload.payload.petId, 'yoonie');
assert.equal(projectionMessage.payload.payload.status, 'working');

await rpc(
  'recall_visit',
  { p_visit_id: visit.id, p_idempotency_key: randomUUID() },
  visitor.token,
);
const endedMessage = await hostChannel.waitForBroadcast('visit_ended');
assert.equal(endedMessage.payload.payload.leaseId, visit.id);

await rpc(
  'update_projection',
  { p_pet_id: 'yoonie', p_skin_id: 'yoonie', p_status: 'idle' },
  visitor.token,
);
await expectNoBroadcast(hostChannel, 'projection_updated');

const revokedChannel = await openPrivateChannel(topic, host.token, { allowDenied: true });
assert.equal(revokedChannel.status, 'error');
revokedChannel.close();
hostChannel.close();

console.log(
  'Line-A Realtime E2E passed: private join, stranger denial, delivery, final event, and revocation',
);
