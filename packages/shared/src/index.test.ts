import { describe, expect, it } from 'vitest';
import * as shared from './index';

describe('@pawbae/shared entrypoint', () => {
  it('exports the runtime contract surface', () => {
    expect(Object.keys(shared).sort()).toEqual(
      [
        'AGENT_SOURCES',
        'APPROVED_SKIN_IDS',
        'DEFAULT_PROJECTION_SKIN_ID',
        'EVENT_KINDS',
        'EVENT_RARITIES',
        'FRIEND_RELATIONS',
        'MEMORY_DURATION_BUCKETS',
        'MEMORY_PARAMETER_LOCALIZATIONS',
        'MEMORY_TEMPLATE_FIXTURES',
        'MEMORY_TEMPLATE_KEYS',
        'MEMORY_TEMPLATE_LOCALIZATIONS',
        'MEMORY_TIMES_OF_DAY',
        'PET_MOODS',
        'PET_SPRITE_STATES',
        'PROJECTION_STATUSES',
        'VISIT_STATUSES',
        'createEggHatchedEvent',
        'createEvent',
        'createMemoryTemplatePayload',
        'createSouvenirFoundEvent',
        'createStreakMilestoneEvent',
        'createTaskCompletedEvent',
        'sanitizePrivatePetSnapshot',
        'sanitizePublicPetProjection',
      ].sort(),
    );
  });
});
