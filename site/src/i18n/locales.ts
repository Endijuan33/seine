// Every page exists in every locale. English is unprefixed; the others live under /<path>/.
// Adding a locale: add it here, add its strings to ui.ts, and translate every published post.
export const LOCALES = [
    { code: 'en', path: null, htmlLang: 'en', ogLocale: 'en_US', endonym: 'English', short: 'EN' },
    { code: 'zh-Hans', path: 'zh-hans', htmlLang: 'zh-Hans', ogLocale: 'zh_CN', endonym: '简体中文', short: '简' },
    { code: 'zh-Hant', path: 'zh-hant', htmlLang: 'zh-Hant', ogLocale: 'zh_TW', endonym: '繁體中文', short: '繁' },
] as const;

export type Locale = (typeof LOCALES)[number]['code'];
export const DEFAULT_LOCALE: Locale = 'en';
export const TRANSLATED_LOCALES = LOCALES.filter((l) => l.code !== DEFAULT_LOCALE).map((l) => l.code);

export function localeDef(code: Locale) {
    return LOCALES.find((l) => l.code === code)!;
}

/** The `[...lang]` route param for a locale (undefined renders at the root). */
export function langParam(code: Locale): string | undefined {
    return localeDef(code).path ?? undefined;
}

export function localeFromParam(param: string | undefined): Locale {
    const found = LOCALES.find((l) => (l.path ?? undefined) === param);
    if (!found) throw new Error(`unknown locale path: ${param}`);
    return found.code;
}

/** Site-relative URL for `path` (which starts and ends with "/") in a locale. */
export function localizePath(path: string, code: Locale): string {
    const prefix = localeDef(code).path;
    return prefix ? `/${prefix}${path}` : path;
}

export const localeStaticPaths = () => LOCALES.map((l) => ({ params: { lang: langParam(l.code) } }));
