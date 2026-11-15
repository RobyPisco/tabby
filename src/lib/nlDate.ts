/**
 * Date scritte a parole in italiano per i promemoria:
 * "tra 2 ore", "tra mezz'ora", "domani alle 9", "lunedì sera", "il 5 ottobre alle 18:30",
 * "5/10 alle 7", "alle 21", "stasera", "dopodomani mattina".
 */

const WEEKDAYS = ["domenica", "lunedi", "martedi", "mercoledi", "giovedi", "venerdi", "sabato"];
const MONTHS = [
  "gennaio", "febbraio", "marzo", "aprile", "maggio", "giugno",
  "luglio", "agosto", "settembre", "ottobre", "novembre", "dicembre",
];
const MINUTE = 60_000;
const UNITS: [RegExp, number][] = [
  [/^(minut[oi]|min)$/, MINUTE],
  [/^(or[ae]|h)$/, 60 * MINUTE],
  [/^giorn[oi]$/, 24 * 60 * MINUTE],
  [/^settiman[ae]$/, 7 * 24 * 60 * MINUTE],
];
/** Parti del giorno: ora predefinita e se un'ora "piccola" va letta dopo mezzogiorno. */
const PARTS: [RegExp, number, boolean][] = [
  [/\b(stamattina|mattina|mattino)\b/, 9, false],
  [/\bmezzogiorno\b/, 12, false],
  [/\bpomeriggio\b/, 15, true],
  [/\b(stasera|sera)\b/, 20, true],
  [/\bmezzanotte\b/, 0, false],
];
/** Senza ora esplicita un giorno vale alle 9. */
const DEFAULT_HOUR = 9;

function normalize(text: string): string {
  return text
    .toLowerCase()
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .replace(/[’`]/g, "'")
    .replace(/\s+/g, " ")
    .trim();
}

function startOfDay(date: Date, addDays = 0): Date {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate() + addDays);
}

export function parseItalianDate(input: string, now: Date = new Date()): Date | null {
  let text = normalize(input);
  if (!text) return null;

  // "tra 2 ore", "fra 10 minuti", "tra un'ora", "tra mezz'ora"
  const relative = /^(?:tra|fra) (un|una|mezz|\d+(?:[.,]\d+)?) ?'? ?([a-z]+)$/.exec(text);
  if (relative) {
    const unit = UNITS.find(([re]) => re.test(relative[2]))?.[1];
    if (!unit) return null;
    const amount =
      relative[1] === "mezz" ? 0.5 : relative[1].startsWith("un") ? 1 : parseFloat(relative[1].replace(",", "."));
    return new Date(now.getTime() + amount * unit);
  }

  let day: Date | null = null;
  const take = (re: RegExp): RegExpExecArray | null => {
    const match = re.exec(text);
    if (match) text = text.replace(match[0], " ");
    return match;
  };

  if (take(/\bdopodomani\b/)) day = startOfDay(now, 2);
  else if (take(/\bdomani\b/)) day = startOfDay(now, 1);
  else if (take(/\boggi\b/)) day = startOfDay(now);

  const weekday = take(new RegExp(`\\b(${WEEKDAYS.join("|")})( prossimo| prossima)?\\b`));
  if (weekday && !day) {
    // Il prossimo giorno con quel nome, da domani in poi.
    const target = WEEKDAYS.indexOf(weekday[1]);
    const ahead = ((target - now.getDay() + 7) % 7) || 7;
    day = startOfDay(now, ahead);
  }

  const named = take(new RegExp(`\\b(?:il )?(\\d{1,2}) (${MONTHS.join("|")})(?: (\\d{4}))?\\b`));
  if (named && !day) {
    const year = named[3] ? Number(named[3]) : now.getFullYear();
    day = new Date(year, MONTHS.indexOf(named[2]), Number(named[1]));
    if (!named[3] && day < startOfDay(now)) day.setFullYear(year + 1);
  }

  const numeric = take(/\b(?:il )?(\d{1,2})\/(\d{1,2})(?:\/(\d{2,4}))?\b/);
  if (numeric && !day) {
    let year = numeric[3] ? Number(numeric[3]) : now.getFullYear();
    if (year < 100) year += 2000;
    day = new Date(year, Number(numeric[2]) - 1, Number(numeric[1]));
    if (!numeric[3] && day < startOfDay(now)) day.setFullYear(year + 1);
  }

  let hour: number | null = null;
  let minute = 0;
  let afternoon = false;
  for (const [re, defaultHour, pm] of PARTS) {
    if (take(re)) {
      hour = defaultHour;
      afternoon = pm;
      break;
    }
  }
  const time = take(/\b(?:alle|all'|ore|verso le|per le)? ?(\d{1,2})(?:[:.](\d{2}))?\b/);
  if (time) {
    hour = Number(time[1]);
    minute = time[2] ? Number(time[2]) : 0;
    if (afternoon && hour < 12) hour += 12;
  }

  // Deve restare solo "di", "il", "alle" e simili: altrimenti non abbiamo capito la frase.
  if (text.replace(/\b(di|del|il|lo|la|alle|ore|a|e|prossimo|prossima)\b/g, "").trim() !== "") return null;
  if (hour === null && day === null) return null;
  if (hour !== null && (hour > 23 || minute > 59)) return null;
  if (day && Number.isNaN(day.getTime())) return null;

  const base = day ?? startOfDay(now);
  const result = new Date(base.getFullYear(), base.getMonth(), base.getDate(), hour ?? DEFAULT_HOUR, minute);
  // Solo l'ora ("alle 9"): se oggi è già passata, è domani.
  if (!day && result <= now) result.setDate(result.getDate() + 1);
  return result;
}

const EN_WEEKDAYS = ["sunday", "monday", "tuesday", "wednesday", "thursday", "friday", "saturday"];
const EN_MONTHS = [
  "january", "february", "march", "april", "may", "june",
  "july", "august", "september", "october", "november", "december",
];
const EN_UNITS: [RegExp, number][] = [
  [/^(minutes?|mins?)$/, MINUTE],
  [/^(hours?|hrs?|h)$/, 60 * MINUTE],
  [/^days?$/, 24 * 60 * MINUTE],
  [/^weeks?$/, 7 * 24 * 60 * MINUTE],
];
const EN_PARTS: [RegExp, number, boolean][] = [
  [/\b(this morning|morning)\b/, 9, false],
  [/\bnoon\b/, 12, false],
  [/\b(this afternoon|afternoon)\b/, 15, true],
  [/\b(tonight|this evening|evening)\b/, 20, true],
  [/\bmidnight\b/, 0, false],
];

/**
 * Le stesse frasi in inglese: "in 2 hours", "in half an hour", "tomorrow at 9", "monday evening",
 * "on 5 october at 6:30pm", "october 5", "at 9pm", "tonight", "the day after tomorrow".
 */
export function parseEnglishDate(input: string, now: Date = new Date()): Date | null {
  let text = normalize(input);
  if (!text) return null;

  const relative = /^in (an?|half an|\d+(?:[.,]\d+)?) ?([a-z]+)$/.exec(text);
  if (relative) {
    const unit = EN_UNITS.find(([re]) => re.test(relative[2]))?.[1];
    if (!unit) return null;
    const amount = relative[1] === "half an" ? 0.5 : relative[1].startsWith("a") ? 1 : parseFloat(relative[1].replace(",", "."));
    return new Date(now.getTime() + amount * unit);
  }

  let day: Date | null = null;
  const take = (re: RegExp): RegExpExecArray | null => {
    const match = re.exec(text);
    if (match) text = text.replace(match[0], " ");
    return match;
  };

  if (take(/\b(the )?day after tomorrow\b/)) day = startOfDay(now, 2);
  else if (take(/\btomorrow\b/)) day = startOfDay(now, 1);
  else if (take(/\btoday\b/)) day = startOfDay(now);

  const weekday = take(new RegExp(`\\b(?:next |this )?(${EN_WEEKDAYS.join("|")})\\b`));
  if (weekday && !day) {
    const target = EN_WEEKDAYS.indexOf(weekday[1]);
    const ahead = ((target - now.getDay() + 7) % 7) || 7;
    day = startOfDay(now, ahead);
  }

  const months = EN_MONTHS.join("|");
  const named =
    take(new RegExp(`\\b(?:on )?(?:the )?(\\d{1,2})(?:st|nd|rd|th)? (?:of )?(${months})(?: (\\d{4}))?\\b`)) ??
    take(new RegExp(`\\b(?:on )?(${months}) (\\d{1,2})(?:st|nd|rd|th)?(?:,? (\\d{4}))?\\b`));
  if (named && !day) {
    const monthFirst = EN_MONTHS.includes(named[1]);
    const dayNumber = Number(monthFirst ? named[2] : named[1]);
    const month = EN_MONTHS.indexOf(monthFirst ? named[1] : named[2]);
    const year = named[3] ? Number(named[3]) : now.getFullYear();
    day = new Date(year, month, dayNumber);
    if (!named[3] && day < startOfDay(now)) day.setFullYear(year + 1);
  }

  let hour: number | null = null;
  let minute = 0;
  let afternoon = false;
  for (const [re, defaultHour, pm] of EN_PARTS) {
    const part = take(re);
    if (part) {
      hour = defaultHour;
      afternoon = pm;
      // "this afternoon", "tonight": oggi, anche se l'ora indicata è già passata.
      if (!day && /^(this|tonight)/.test(part[0])) day = startOfDay(now);
      break;
    }
  }
  const time = take(/\b(?:at |around |by )?(\d{1,2})(?:[:.](\d{2}))? ?(am|pm)?\b/);
  if (time) {
    hour = Number(time[1]);
    minute = time[2] ? Number(time[2]) : 0;
    if (time[3] === "pm" && hour < 12) hour += 12;
    else if (time[3] === "am" && hour === 12) hour = 0;
    else if (!time[3] && afternoon && hour < 12) hour += 12;
  }

  if (text.replace(/\b(at|on|the|of|in|next|this)\b/g, "").trim() !== "") return null;
  if (hour === null && day === null) return null;
  if (hour !== null && (hour > 23 || minute > 59)) return null;
  if (day && Number.isNaN(day.getTime())) return null;

  const base = day ?? startOfDay(now);
  const result = new Date(base.getFullYear(), base.getMonth(), base.getDate(), hour ?? DEFAULT_HOUR, minute);
  if (!day && result <= now) result.setDate(result.getDate() + 1);
  return result;
}

/** Data scritta a parole, in italiano o in inglese. */
export function parseDate(input: string, now: Date = new Date()): Date | null {
  return parseItalianDate(input, now) ?? parseEnglishDate(input, now);
}
