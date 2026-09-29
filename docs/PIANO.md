# Piano: Tabby, sticky notes a bordo schermo per Windows

## Context
L'idea: sticky notes agganciate al bordo destro dello schermo: una "pillola" sottile a riposo, le note si aprono a ventaglio quando il cursore si avvicina, clic = editor a piena dimensione. Dati locali (SQLite), archivio invece di elimina, ricerca, export Markdown/txt, scorciatoie globali. Nessun account/telemetria.
Obiettivo: un'app personale per Windows 11, solo per uso proprio (niente licenze, niente cifratura obbligatoria, niente cloud).

Risposta alla domanda: sì, è del tutto fattibile. La parte più delicata è la finestra "docked" (sempre in primo piano, trasparente, senza bordi, che reagisce alla prossimità del mouse), ma è ben supportata su Windows.

## Approccio consigliato
**Tauri 2 (Rust) + Svelte 5 + Vite** (stesso stack Svelte già usato in "La tua stella"; eseguibile ~10 MB, avvio rapido, poca RAM).
Alternativa scartata: Electron (più pesante), WPF/WinUI (effetti animati del ventaglio più laboriosi).

Posizione progetto: `D:\progetti\tabby` (C: è stretto).

### Funzionalità (MVP → extra)
1. **Finestra dock**: senza bordi, trasparente, always-on-top, `skip_taskbar`, alta quanto la work area, ancorata al bordo destro (lato configurabile). Tre stati: pillola / ventaglio / editor.
1b. **Dettaglio dei tre stati**:
   - *A riposo*: pillola sottile (~12 px) sul bordo, un trattino colorato per nota; nessuna finestra visibile né icona in taskbar.
   - *Ventaglio*: all'avvicinarsi del cursore le note scendono a "scandole" sfalsate di ~45 ms l'una dall'altra; ogni linguetta verticale mostra l'etichetta (titolo) ruotata e mantiene il suo colore. In fondo un pulsante "+" per nuova nota.
   - *Nota aperta*: la nota scorre fuori dal deck a dimensione piena, leggibile per intero; il deck resta visibile dietro con le altre linguette.
   - *Salvataggio*: debounce di 250 ms dopo l'ultimo tasto, scrittura su disco automatica.
1c. **Finestra "Tutte le note"** (scorciatoia globale, es. `Ctrl+Alt+L`): finestra normale a due pannelli.
   - Sinistra: campo di ricerca con contatore ("9 notes"), filtri Tutte / Attive / Archiviate, pulsante Importa; ogni riga ha checkbox di selezione multipla, barra colorata, titolo, anteprima su una riga, badge ATTIVA/ARCHIVIATA e tempo relativo (23m, 1h, 15h).
   - Destra: anteprima/editor della nota selezionata con stato ("Attiva · nel deck"), pulsanti Segna completata, Esporta…, Elimina, date di creazione e ultima modifica.
   - Azioni di gruppo sulle note selezionate (archivia, esporta, elimina).
   - Font "a mano" per il corpo delle note (stile sticky), UI pulita per il resto.
2. **Prossimità**: polling del cursore lato Rust (`GetCursorPos`) a ~60 Hz, evento al frontend quando entra nella fascia di bordo; la finestra è click-through (`set_ignore_cursor_events`) a riposo così non intralcia le altre app.
3. **Note**: colore, titolo dalla prima riga, editor testo/Markdown, autosave.
4. **Archivio** al posto di elimina, **ricerca** su tutte le note (SQLite FTS5).
5. **Scorciatoie globali** (plugin `global-shortcut`): nuova nota, mostra/nascondi deck, ricerca.
6. **Tray icon + avvio con Windows** (plugin `autostart`).
7. **Export**: Markdown, txt, file unico. Storage: SQLite (`rusqlite`/plugin `sql`) in `%APPDATA%`; opzionale una cartella di file `.md` per sincronizzare via OneDrive/Drive.
8. **Promemoria** su ogni nota:
   - Data/ora di scadenza (anche ricorrente: giornaliera, settimanale, mensile) e "posticipa" (snooze 10 min / 1 h / domani).
   - Notifica toast nativa di Windows (plugin `notification`) con azioni Posticipa / Fatto; clic sulla notifica apre la nota.
   - Scheduler in Rust (thread con timer sulla prossima scadenza, ricalcolato a ogni modifica) che funziona finché l'app è in tray; i promemoria scaduti mentre il PC era spento/app chiusa vengono mostrati all'avvio.
   - Sulla nota: badge/icona campanella nella pillola e nel ventaglio; la nota con promemoria scaduto pulsa o cambia colore per attirare l'attenzione.
   - Inserimento rapido: campo data/ora nell'editor + scorciatoia globale "nuova nota con promemoria" (opzionale: linguaggio naturale tipo "domani alle 9").
   - Tabella `reminders` in SQLite (note_id, due_at, repeat_rule, snoozed_until, done_at).
9. **Editor ricco**: checklist con caselle spuntabili, Markdown live (CodeMirror 6 o TipTap), incolla immagini/screenshot (salvate in `%APPDATA%\it.pisco.tabby\media`, referenziate dalla nota).
10. **Organizzazione**: tag e cartelle, note fissate in alto, cestino con ripristino (oltre all'archivio), link tra note `[[titolo]]` con autocompletamento e backlink, cronologia versioni per nota.
11. **Cattura rapida**: scorciatoia globale che salva il testo negli appunti come nuova nota (con app/finestra di origine), drag&drop di file e link sul deck.
12. Extra futuri (non in questo giro): backup automatico su D:, PIN/cifratura AES-GCM, sync via cartella cloud,  cifratura AES-GCM, monitor multipli, tema chiaro/scuro.

### Punti critici da verificare presto
- Trasparenza + click-through su Windows 11 con WebView2 (spike iniziale).
- Comportamento sopra app a schermo intero (si accetta l'always-on-top standard di Windows).
- DPI/scaling e monitor multipli.

## Passi di implementazione
1. Verificare prerequisiti: Rust (rustup, MSVC Build Tools), Node, WebView2 (già su Win11).
2. `npm create tauri-app` (Svelte + TS) in `D:\progetti\tabby`.
3. Spike: finestra trasparente ancorata a destra + rilevamento prossimità + animazione pillola→ventaglio.
4. Livello dati (SQLite + FTS5) e store Svelte per le note.
5. Editor a piena dimensione con autosave; colori; archivio; checklist, Markdown, immagini incollate.
5b. Tag, cartelle, cestino, link `[[nota]]` + backlink, cronologia versioni; cattura rapida da appunti e drag&drop.
6. Scorciatoie globali, tray, autostart.
7. Promemoria: tabella + scheduler Rust + notifiche toast con Posticipa/Fatto + recupero dei scaduti all'avvio.
8. Export e impostazioni (lato, larghezza, colori, suono notifica).
9. Build installer (`npm run tauri build` → MSI/NSIS).

## Verifica
- `npm run tauri dev`: pillola visibile a destra, ventaglio all'avvicinarsi del mouse, click su altre app non bloccati a riposo.
- Creare/modificare/archiviare/cercare note; riavviare e controllare la persistenza.
- Scorciatoie globali funzionanti con altre app in primo piano.
- Impostare un promemoria a 1 minuto: compare il toast, Posticipa lo sposta, Fatto lo chiude; chiudere l'app con un promemoria in scadenza e riaprirla: viene mostrato come scaduto; testare una ricorrenza giornaliera.
- Build release e test installer su questo portatile.
