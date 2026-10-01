# Da fare

Ordine consigliato: prima i punti che rendono l'app usabile ogni giorno, poi le funzioni extra.
Dettagli di progetto e motivazioni in [`PIANO.md`](PIANO.md).

## 0. Da verificare / sistemare subito
- [x] Provare a mano lo spike: pillola, ventaglio, nota aperta (verificato il 2026-09-29).
- [x] Editing e persistenza: il testo scritto si ritrova dopo il riavvio dell'app (verificato il 2026-09-29).
- [ ] Ancora da provare a mano: uscita dopo ~0,35 s e click-through a riposo.
- [x] Altezza del deck con molte note: le linguette si accorciano fino a 36 px per restare nello schermo (oltre ~30 note servirà uno scroll).
- [x] Pillola più grande (20 px) con colori saturi derivati da quelli delle note.
- [x] Monitor multipli e scaling DPI: scelta dello schermo nelle impostazioni; il deck si riposiziona da solo se cambiano monitor, risoluzione, scala o barra delle applicazioni.
- [ ] Da provare a mano: comportamento sopra app a schermo intero (always-on-top standard, se dà fastidio c'è `Ctrl+Alt+H`).
- [x] Avviso del linker `linker_messages` silenziato in `Cargo.toml`.

## 1. Base dell'app
- [x] Ricerca su tutte le note (SQLite FTS5, per prefisso e senza accenti) con contatore risultati.
- [x] Finestra **"Tutte le note"** (`Ctrl+Alt+L` o pulsante ☰ nel deck): elenco con ricerca, filtri Tutte/Attive/Archiviate, checkbox, badge di stato e tempo relativo; dettaglio modificabile con Segna completata, Elimina (con conferma), date. Si sincronizza col deck.
- [x] Pulsante **Esporta…** nel dettaglio (.md o .txt).
- [x] Azioni di gruppo sulle note selezionate: archivia, ripristina, elimina.
- [x] Azione di gruppo "Esporta…" (un file per nota o tutte in un file) e pulsante Importa (.md/.txt).
- [x] Ripristino dall'archivio.
- [x] **Cestino** con ripristino, "Elimina per sempre" e Svuota; le note nel cestino da più di 30 giorni vengono eliminate all'avvio.
- [x] Selettore colore per nota (nel deck e in "Tutte le note").
- [x] Scorciatoie globali: `Ctrl+Alt+L` tutte le note, `Ctrl+Alt+N` nuova nota nel deck, `Ctrl+Alt+H` mostra/nascondi deck, `Ctrl+Alt+P` nuova nota con promemoria. Se un'altra app ne occupa una, viene saltata senza bloccare l'avvio (`Ctrl+Alt+R` era già occupata).
- [x] Icona nella tray: clic sinistro apre "Tutte le note"; menu con tutte le note, nuova nota, mostra/nascondi deck, Avvia con Windows, Esci.
- [x] Avvio automatico con Windows (plugin `autostart`), attivabile dal menu della tray o dalle impostazioni (conviene attivarlo dall'app installata: in sviluppo punterebbe all'eseguibile di debug).
- [x] Un'unica istanza dell'app (plugin `single-instance`): un secondo avvio apre "Tutte le note".

## 2. Promemoria
- [x] Tabella `reminders` (note_id, due_at, repeat, snoozed_until, fired_at): un promemoria per nota; il posticipo non sposta la serie.
- [x] Campanella nell'editor (deck e "Tutte le note"): scelte rapide (tra 1 ora, stasera, domani, lunedì), data/ora, ricorrenza giornaliera/settimanale/mensile.
- [x] Scheduler in Rust (`reminders.rs`) che dorme fino alla prossima scadenza (massimo 30 s) e si risveglia a ogni modifica; ricorrenze in ora locale, con test.
- [x] Notifiche toast native (`tauri-winrt-notification`, il plugin `notification` su desktop non ha pulsanti) con **10 min / 1 ora / Domani / Fatto**; clic sulla notifica apre la nota nel deck.
- [x] Con l'app installata le notifiche mostrano nome e icona dell'app (verificato il 2026-09-29; in sviluppo appaiono come "Windows PowerShell").
- [x] Promemoria scaduti mentre l'app era chiusa: notificati all'avvio (oltre 3 insieme, una notifica di riepilogo).
- [x] Campanella sulla linguetta e nell'elenco; se scaduto lampeggiano di rosso linguetta e trattino della pillola, e nella nota compare la barra "Scaduto" con 10 min / Domani / Fatto.
- [x] Scorciatoia "nuova nota con promemoria" (`Ctrl+Alt+P`, anche dal menu della tray).
- [x] Data in linguaggio naturale nel riquadro del promemoria ("domani alle 9", "tra 2 ore", "lunedì sera", "il 5 ottobre alle 18"), con test (`npm test`).

## 3. Editor ricco
- [x] Checklist con caselle spuntabili: pulsante ☑ o `Ctrl+L`, scrivere `[] ` a inizio riga, `Ctrl+Invio` spunta; avanzamento "2/5" nell'elenco.
- [x] Markdown live con CodeMirror 6: il testo resta Markdown, i simboli spariscono fuori dalla riga del cursore; `Ctrl+B` / `Ctrl+I` / `Ctrl+Shift+X`; Invio continua elenchi e caselle.
- [x] Incolla (`Ctrl+V`) o trascina immagini/screenshot, salvate in `%APPDATA%\it.pisco.tabby\media` con nome dal contenuto (niente doppioni).
- [x] All'avvio si eliminano dalla cartella `media` i file non più citati da nessuna nota (cestino e cronologia compresi).
- [x] Modelli rapidi nelle note vuote: Lista spesa, Da fare, Riunione, Scaletta.

## 4. Organizzazione
- [x] Tag scritti nel testo (`#parola`, con autocompletamento) e cartelle (colonna a sinistra in "Tutte le note": crea, doppio clic per rinominare, elimina; sposta una o più note).
- [x] Note fissate in alto nel deck e nell'elenco (pulsante con la puntina).
- [x] Link tra note `[[titolo]]` con autocompletamento; clic per aprire (o creare) la nota; "Citata in" con i link entranti.
- [x] Quando si rinomina una nota, i link `[[vecchio titolo]]` nelle altre diventano `[[nuovo titolo]]`.
- [x] Cronologia versioni per nota (al massimo una ogni 10 minuti, ultime 50) con anteprima e Ripristina.

## 5. Cattura rapida
- [x] `Ctrl+Alt+V` (o menu della tray) salva il testo negli appunti come nuova nota, con la finestra/app di origine; notifica con clic per aprirla.
- [x] Trascinare file, link o testo sul deck crea una nota; dentro una nota aperta i file diventano immagini o allegati 📎 (salvati in `media`, si aprono con un clic). Link web cliccabili nel testo.
- [ ] Da provare a mano: il trascinamento (non automatizzabile dai test).

## 6. Export e impostazioni
- [x] Export Markdown o testo, un file per nota (con le immagini in `media/`) o file unico; "Esporta tutto…" e "Importa…" nella colonna di "Tutte le note".
- [x] Impostazioni (colonna di "Tutte le note" o menu della tray): bordo destro/sinistro, larghezza della nota, schermo, carattere e dimensione del testo, colore delle note nuove, suono delle notifiche, tema chiaro/scuro/automatico, avvio con Windows. Salvate in `settings.json`.

## 7. Rifinitura e rilascio
- [x] Icona definitiva (`assets/app-icon.png`), versione 1.0.0, descrizione e installer in italiano.
- [x] Build installer (`npm run tauri build` → `bundle\nsis\…-setup.exe` e `bundle\msi\…_it-IT.msi`) e prova su questo portatile (installata per l'utente il 2026-09-29).
- [ ] Prova sul portatile di casa.
- [x] Test: `cargo test` (strato dati, promemoria e ricorrenze, tag, link, export, impostazioni, cattura) e `npm test` (date a parole).

## 8. Idee per dopo
- [ ] Backup automatico su D: o su cartella cloud; sync via OneDrive/Drive (anche una cartella di file `.md` sincronizzabile).
- [ ] Blocco con PIN e cifratura AES-GCM per singole note.
- [ ] Note che compaiono in base all'app in primo piano (es. Mixing Station).
- [ ] Timer/pomodoro legato a una nota.
- [ ] Dettatura vocale (`Win+H`) o trascrizione locale con Whisper.
- [ ] Riassunto o riscrittura di una nota con Claude (chiave API).

## 9. Google Calendar
- [x] Integrazione Google Calendar → note in sola lettura con promemoria.
  - Flusso OAuth2 PKCE: apre il browser, cattura il callback su `127.0.0.1`.
  - Il token è salvato in `%APPDATA%\it.pisco.tabby\gcal_token.json`.
  - Sync ogni 15 min; cartella dedicata "Calendario Google"; finestra 1–2 settimane.
  - Evento rimosso da Calendar → nota nel cestino; nota eliminata/archiviata dall'utente → non torna.
  - Note di sola lettura (deck e "Tutte le note"), con badge, orario dell'evento e "Elimina" nel deck.
- [x] Client Secret fuori dal repo: variabile di build `TABBY_GOOGLE_CLIENT_SECRET` (secret `GOOGLE_CLIENT_SECRET` nel workflow).
- [ ] App Google: pubblicata "In produzione" senza verifica (tetto 100 utenti, avviso "app non verificata").
- [x] Reti con proxy aziendale (PAC + autenticazione NTLM/Negotiate): le chiamate a Google passano da WinHTTP (`http.rs`), che usa proxy di sistema, credenziali di Windows e certificati dell'archivio. Provato su una rete con proxy PAC + NTLM. Opzione «Usa il proxy di Windows» spenta di default (accesa per chi aveva già usato Calendar).
- [ ] Calendari deselezionati: le note già importate restano.
