export type UnknownRecord = Record<string, unknown>;

export const SAFE_ID_PATTERN = /^[a-z0-9][a-z0-9._-]{0,63}$/;

export function assertPlainRecord(value: unknown, label: string): UnknownRecord {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw new TypeError(`${label} must be an object`);
  }
  const prototype = Object.getPrototypeOf(value);
  if (prototype !== Object.prototype && prototype !== null) {
    throw new TypeError(`${label} must be a plain object`);
  }
  return value as UnknownRecord;
}

export function assertExactRecord(
  value: unknown,
  expectedKeys: readonly string[],
  label: string,
): UnknownRecord {
  const record = assertPlainRecord(value, label);
  const actualKeys = Object.keys(record);
  const unknownKeys = actualKeys.filter((key) => !expectedKeys.includes(key));
  if (unknownKeys.length > 0) {
    throw new TypeError(`${label} contains unknown key: ${unknownKeys.join(', ')}`);
  }
  const missingKeys = expectedKeys.filter((key) => !Object.hasOwn(record, key));
  if (missingKeys.length > 0) {
    throw new TypeError(`${label} is missing required key: ${missingKeys.join(', ')}`);
  }
  return record;
}

export function assertEnum<const T extends readonly string[]>(
  value: unknown,
  allowed: T,
  label: string,
): T[number] {
  if (typeof value !== 'string' || !allowed.includes(value)) {
    throw new TypeError(`${label} must be one of: ${allowed.join(', ')}`);
  }
  return value as T[number];
}

export function assertBoundedInteger(
  value: unknown,
  minimum: number,
  maximum: number,
  label: string,
): number {
  if (!Number.isInteger(value) || (value as number) < minimum || (value as number) > maximum) {
    throw new TypeError(`${label} must be an integer from ${minimum} through ${maximum}`);
  }
  return value as number;
}

export function assertBoolean(value: unknown, label: string): boolean {
  if (typeof value !== 'boolean') {
    throw new TypeError(`${label} must be a boolean`);
  }
  return value;
}

export function assertSafeId(value: unknown, label: string): string {
  if (typeof value !== 'string' || !SAFE_ID_PATTERN.test(value)) {
    throw new TypeError(`${label} must match ${SAFE_ID_PATTERN.source}`);
  }
  return value;
}

export function assertDisplayName(value: unknown, label: string): string {
  if (typeof value !== 'string') {
    throw new TypeError(`${label} must be a string`);
  }
  const trimmed = value.trim();
  const length = [...trimmed].length;
  const unsafeFormatting = /[\u0000-\u001f\u007f-\u009f\u202a-\u202e\u2066-\u2069]/u;
  if (length < 1 || length > 64 || unsafeFormatting.test(trimmed)) {
    throw new TypeError(`${label} must contain 1 to 64 characters without control characters`);
  }
  return trimmed;
}

export function assertIsoTimestamp(value: unknown, label: string): string {
  const timestampPattern =
    /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(?:\.(\d{1,6}))?(Z|[+-](\d{2}):(\d{2}))$/;
  const match = typeof value === 'string' ? timestampPattern.exec(value) : null;
  if (!match) {
    throw new TypeError(`${label} must be an ISO 8601 timestamp`);
  }

  const timestamp = value as string;
  const year = Number(match[1]);
  const month = Number(match[2]);
  const day = Number(match[3]);
  const hour = Number(match[4]);
  const minute = Number(match[5]);
  const second = Number(match[6]);
  const zone = match[8];
  const calendarYear = year < 100 ? year + 400 : year;
  const daysInMonth = new Date(Date.UTC(calendarYear, month, 0)).getUTCDate();
  const offsetHour = zone === 'Z' ? 0 : Number(match[9]);
  const offsetMinute = zone === 'Z' ? 0 : Number(match[10]);
  const validCalendar =
    year >= 1 &&
    month >= 1 &&
    month <= 12 &&
    day >= 1 &&
    day <= daysInMonth &&
    hour <= 23 &&
    minute <= 59 &&
    second <= 59;
  const validOffset = offsetHour <= 14 && offsetMinute <= 59 && !(offsetHour === 14 && offsetMinute > 0);
  if (!validCalendar || !validOffset) {
    throw new TypeError(`${label} must be an ISO 8601 timestamp`);
  }

  const epoch = Date.parse(timestamp);
  if (!Number.isFinite(epoch)) {
    throw new TypeError(`${label} must be an ISO 8601 timestamp`);
  }
  return new Date(epoch).toISOString();
}

export function deepFreeze<T>(value: T): Readonly<T> {
  if (typeof value !== 'object' || value === null) {
    return value;
  }

  if (Array.isArray(value)) {
    const clone = value.map((child) => deepFreeze(child));
    return Object.freeze(clone) as unknown as Readonly<T>;
  }

  const clone = Object.fromEntries(
    Object.entries(value).map(([key, child]) => [key, deepFreeze(child)]),
  );
  return Object.freeze(clone) as Readonly<T>;
}
