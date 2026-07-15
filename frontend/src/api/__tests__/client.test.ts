import { getApiErrorMessage } from '../client';

describe('getApiErrorMessage', () => {
  it.each([
    [{ message: 'message error' }, 'message error'],
    [{ detail: 'detail error' }, 'detail error'],
    [{ error: { message: 'nested error' } }, 'nested error'],
    ['plain text error', 'plain text error'],
    ['   ', 'fallback'],
  ])('normalizes JSON envelopes and text responses', (payload, expected) => {
    expect(getApiErrorMessage(payload, 'fallback')).toBe(expected);
  });
});
