import { describe, expect, it } from 'vitest';

import { contentDiscoveryPathFromDeepLink } from '@/lib/deepLink';

describe('contentDiscoveryPathFromDeepLink', () => {
  it('converts capture URL and note into content discovery route', () => {
    expect(
      contentDiscoveryPathFromDeepLink(
        'pkos://capture?url=https%3A%2F%2Fexample.com%2Fa&note=%E7%A8%8D%E5%90%8E%E9%98%85%E8%AF%BB',
      ),
    ).toBe('/discover?url=https%3A%2F%2Fexample.com%2Fa&note=%E7%A8%8D%E5%90%8E%E9%98%85%E8%AF%BB');
  });

  it('rejects unrelated or empty deep links', () => {
    expect(contentDiscoveryPathFromDeepLink('https://example.com')).toBeNull();
    expect(contentDiscoveryPathFromDeepLink('pkos://capture')).toBeNull();
  });
});
