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
    throw new Error(`${method} ${path} failed (${response.status}): ${text}`);
  }
  return { data, response };
}

async function signUp(label) {
  const suffix = randomUUID().replaceAll('-', '').slice(0, 12);
  const handle = `api-${label}-${suffix}`;
  const { data } = await request('/auth/v1/signup', {
    method: 'POST',
    body: {
      email: `${handle}@example.test`,
      password: `T3st-${randomUUID()}`,
      data: { user_name: handle },
    },
  });
  assert.equal(typeof data.access_token, 'string');
  assert.equal(typeof data.user?.id, 'string');
  return { handle, id: data.user.id, token: data.access_token };
}

async function rpc(name, body, token) {
  const { data } = await request(`/rest/v1/rpc/${name}`, { body, method: 'POST', token });
  return Array.isArray(data) ? data[0] : data;
}

async function rpcAttempt(name, body, token) {
  const result = await request(`/rest/v1/rpc/${name}`, {
    allowFailure: true,
    body,
    method: 'POST',
    token,
  });
  return { ...result, row: Array.isArray(result.data) ? result.data[0] : result.data };
}

async function befriend(requester, receiver) {
  await rpc('send_friend_request', { p_target_user_id: receiver.id }, requester.token);
  return rpc(
    'accept_friend_request',
    { p_requester_user_id: requester.id },
    receiver.token,
  );
}

async function waitForVisitStatus(visitId, expectedStatus, token) {
  const deadline = Date.now() + 75_000;
  while (Date.now() < deadline) {
    const { data } = await request(`/rest/v1/visits?id=eq.${visitId}&select=status`, { token });
    if (data[0]?.status === expectedStatus) return;
    await new Promise((resolve) => setTimeout(resolve, 2_000));
  }
  throw new Error(`visit ${visitId} did not reach ${expectedStatus} before the cron deadline`);
}

const alice = await signUp('alice');
const bob = await signUp('bob');

const { data: aliceProfile } = await request(
  `/rest/v1/profiles?id=eq.${alice.id}&select=handle,avatar_url`,
  { token: alice.token },
);
assert.match(aliceProfile[0].handle, /^user-[0-9a-f]{32}$/);
assert.notEqual(aliceProfile[0].handle, alice.handle);
assert.equal(aliceProfile[0].avatar_url, null);

const anonymousProfiles = await request('/rest/v1/profiles?select=id&limit=1', {
  allowFailure: true,
});
assert.ok([401, 403].includes(anonymousProfiles.response.status));

const { data: alicePets } = await request('/rest/v1/pets?select=user_id', {
  token: alice.token,
});
assert.deepEqual(alicePets.map(({ user_id: userId }) => userId), [alice.id]);

const forgedPetWrite = await request(`/rest/v1/pets?user_id=eq.${bob.id}`, {
  body: {
    snapshot: {
      petId: 'yoonie',
      spriteState: 'work',
      mood: 'focused',
      hunger: 80,
      level: 2,
      streak: 1,
      away: false,
    },
  },
  method: 'PATCH',
  token: alice.token,
});
assert.deepEqual(forgedPetWrite.data, []);

const invalidEvent = await request('/rest/v1/events', {
  allowFailure: true,
  body: { kind: 'task_completed', params: { source: 'cc', prompt: 'private' } },
  method: 'POST',
  token: alice.token,
});
assert.equal(invalidEvent.response.status, 400);

const validEvent = await request('/rest/v1/events', {
  body: { kind: 'task_completed', params: { source: 'cc' } },
  method: 'POST',
  token: alice.token,
});
assert.equal(validEvent.response.status, 201);

await rpc('connector_heartbeat', {}, alice.token);
const waitlistEmail = `api-${randomUUID()}@example.test`;
const firstWaitlist = await rpc('join_waitlist', { p_email: waitlistEmail });
const replayedWaitlist = await rpc('join_waitlist', { p_email: waitlistEmail });
assert.equal(firstWaitlist.id, replayedWaitlist.id);

const friendship = await rpc(
  'send_friend_request',
  { p_target_user_id: bob.id },
  alice.token,
);
assert.equal(friendship.status, 'pending');
const acceptedFriendship = await rpc(
  'accept_friend_request',
  { p_requester_user_id: alice.id },
  bob.token,
);
assert.equal(acceptedFriendship.status, 'accepted');

const requestKey = randomUUID();
const visit = await rpc(
  'request_visit',
  { p_host_user_id: bob.id, p_idempotency_key: requestKey },
  alice.token,
);
const replayedVisit = await rpc(
  'request_visit',
  { p_host_user_id: bob.id, p_idempotency_key: requestKey },
  alice.token,
);
assert.equal(visit.id, replayedVisit.id);

const acceptedVisit = await rpc(
  'accept_visit',
  { p_visit_id: visit.id, p_idempotency_key: randomUUID() },
  bob.token,
);
assert.equal(acceptedVisit.status, 'accepted');
assert.equal(
  Date.parse(acceptedVisit.ends_at) - Date.parse(acceptedVisit.started_at),
  30 * 60 * 1000,
);

await rpc(
  'update_projection',
  { p_pet_id: 'yoonie', p_skin_id: 'yoonie', p_status: 'working' },
  alice.token,
);
const { data: visibleProjection } = await request(
  `/rest/v1/pet_projections?owner_user_id=eq.${alice.id}&select=owner_user_id`,
  { token: bob.token },
);
assert.equal(visibleProjection.length, 1);

const recalledVisit = await rpc(
  'recall_visit',
  { p_visit_id: visit.id, p_idempotency_key: randomUUID() },
  alice.token,
);
assert.equal(recalledVisit.status, 'returning');
const { data: revokedProjection } = await request(
  `/rest/v1/pet_projections?owner_user_id=eq.${alice.id}&select=owner_user_id`,
  { token: bob.token },
);
assert.deepEqual(revokedProjection, []);

await waitForVisitStatus(visit.id, 'recalled', alice.token);
const memoryKey = randomUUID();
const memory = await rpc(
  'settle_shared_memory',
  { p_visit_id: visit.id, p_idempotency_key: memoryKey },
  alice.token,
);
const replayedMemory = await rpc(
  'settle_shared_memory',
  { p_visit_id: visit.id, p_idempotency_key: memoryKey },
  alice.token,
);
assert.equal(memory.id, replayedMemory.id);
assert.equal(memory.visit_id, visit.id);
assert.equal(typeof memory.template_key, 'string');
assert.equal(Object.hasOwn(memory, 'rendered_text'), false);

const followupVisit = await rpc(
  'request_visit',
  { p_host_user_id: bob.id, p_idempotency_key: randomUUID() },
  alice.token,
);
await rpc(
  'cancel_visit',
  { p_visit_id: followupVisit.id, p_idempotency_key: randomUUID() },
  alice.token,
);
await rpc('block_user', { p_target_user_id: bob.id }, alice.token);
const { data: blockedProfile } = await request(
  `/rest/v1/profiles?id=eq.${alice.id}&select=id`,
  { token: bob.token },
);
assert.deepEqual(blockedProfile, []);

const host = await signUp('host');
const visitorOne = await signUp('visitor-one');
const visitorTwo = await signUp('visitor-two');
await befriend(visitorOne, host);
await befriend(visitorTwo, host);

const concurrentRequestKey = randomUUID();
const duplicateRequests = await Promise.all([
  rpcAttempt(
    'request_visit',
    { p_host_user_id: host.id, p_idempotency_key: concurrentRequestKey },
    visitorOne.token,
  ),
  rpcAttempt(
    'request_visit',
    { p_host_user_id: host.id, p_idempotency_key: concurrentRequestKey },
    visitorOne.token,
  ),
]);
assert.equal(duplicateRequests.filter(({ response }) => response.ok).length, 2);
assert.equal(duplicateRequests[0].row.id, duplicateRequests[1].row.id);

const competingVisit = await rpc(
  'request_visit',
  { p_host_user_id: host.id, p_idempotency_key: randomUUID() },
  visitorTwo.token,
);
const competingAccepts = await Promise.all([
  rpcAttempt(
    'accept_visit',
    { p_visit_id: duplicateRequests[0].row.id, p_idempotency_key: randomUUID() },
    host.token,
  ),
  rpcAttempt(
    'accept_visit',
    { p_visit_id: competingVisit.id, p_idempotency_key: randomUUID() },
    host.token,
  ),
]);
assert.equal(competingAccepts.filter(({ response }) => response.ok).length, 1);

const competingRows = await Promise.all(
  [duplicateRequests[0].row.id, competingVisit.id].map(async (visitId) => {
    const { data } = await request(`/rest/v1/visits?id=eq.${visitId}&select=id,status`, {
      token: host.token,
    });
    return data[0];
  }),
);
const activeVisitStatuses = new Set(['accepted', 'traveling', 'visiting']);
assert.equal(competingRows.filter(({ status }) => activeVisitStatuses.has(status)).length, 1);
assert.equal(competingRows.filter(({ status }) => status === 'requested').length, 1);

const acceptedRow = competingRows.find(({ status }) => activeVisitStatuses.has(status));
const pendingRow = competingRows.find(({ status }) => status === 'requested');
await rpc(
  'decline_visit',
  { p_visit_id: pendingRow.id, p_idempotency_key: randomUUID() },
  host.token,
);
await rpc(
  'end_visit',
  { p_visit_id: acceptedRow.id, p_idempotency_key: randomUUID() },
  host.token,
);

const raceRequester = await signUp('race-requester');
const raceBlocker = await signUp('race-blocker');
await befriend(raceRequester, raceBlocker);
const [racedRequest, racedBlock] = await Promise.all([
  rpcAttempt(
    'request_visit',
    { p_host_user_id: raceBlocker.id, p_idempotency_key: randomUUID() },
    raceRequester.token,
  ),
  rpcAttempt('block_user', { p_target_user_id: raceRequester.id }, raceBlocker.token),
]);
assert.equal(racedBlock.response.ok, true);
assert.ok([200, 400].includes(racedRequest.response.status));

const { data: racedVisits } = await request(
  `/rest/v1/visits?visitor_user_id=eq.${raceRequester.id}&select=status`,
  { token: raceRequester.token },
);
const unfinishedStatuses = new Set(['requested', 'accepted', 'traveling', 'visiting', 'returning']);
assert.equal(racedVisits.some(({ status }) => unfinishedStatuses.has(status)), false);
const { data: racedFriendship } = await request(
  `/rest/v1/friendships?or=(user_a.eq.${raceRequester.id},user_b.eq.${raceRequester.id})&select=user_a`,
  { token: raceRequester.token },
);
assert.deepEqual(racedFriendship, []);

console.log(
  'Line-A API E2E passed: RLS, validation, social, concurrency, revocation, and memory replay',
);
