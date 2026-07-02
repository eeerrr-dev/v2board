// ja-JP never localized the backend error strings: the old full translation
// tree spread en-US and overrode only UI namespaces, so its errors slice was
// the en-US object. Keep that behavior explicit.
export { default } from './en-US';
