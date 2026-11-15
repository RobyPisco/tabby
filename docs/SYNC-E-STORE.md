# Tabby condivisa tra più PC e Microsoft Store

*Piano del 15/11/2026 · ramo `microsoft`, cartella `D:\progetti\tabby-microsoft`*

Obiettivo: installo Tabby su 3 PC e tutti e tre vedono le stesse note. Poi pubblicazione sul Microsoft Store. Tutto **gratuito anche per chi la sviluppa**: nessun server, nessun abbonamento.

## Passo 1 — Togliere Google Calendar ✅

Fatto il 15/11/2026: codice (`gcal.rs`, `http.rs`, `CalendarTile.svelte`), comandi, impostazioni, interfaccia, secret del workflow e parte dell'informativa sulla privacy.
All'apertura di un database vecchio le note importate dal calendario (copie degli eventi) vengono eliminate, insieme alla cartella «Calendario Google» se resta vuota (`db::remove_google_calendar`). Le vecchie chiavi `gcal_*` di `settings.json` vengono ignorate.

## Passo 2 — Sincronizzazione tramite una cartella a scelta ✅

Fatto il 15/11/2026: `syncdb.rs` (uid, `changed_at`, lapidi, unione delle note, 10 test), `sync.rs` (cartella condivisa, file per PC, immagini, PIN comune, 2 test), sezione «Sincronizzazione» nelle impostazioni. Provato con due copie dell'app e una cartella condivisa finta: note nuove e modifiche passano nei due sensi, le note di benvenuto non si duplicano.
**Attenzione:** le note protette da PIN nella cartella condivisa sono leggibili come nel database locale (il PIN le nasconde solo sullo schermo, ADR-0001). Il PIN vale su tutti i PC: vince quello impostato per ultimo.
Da provare a mano: 3 PC veri con Google Drive, immagini e allegati, PC spento per giorni.


Nessun server: l'utente sceglie una cartella che il suo servizio cloud tiene già sincronizzata. **Funziona con qualsiasi servizio**: Google Drive per desktop (es. `G:\Il mio Drive\Tabby`), OneDrive, Dropbox, una cartella di rete o una chiavetta.

### Regole
- **Il database SQLite non va mai nella cartella condivisa**: i servizi cloud lo rovinerebbero. Resta locale; nella cartella viaggiano solo file di modifiche.
- **Ogni PC scrive solo i propri file** (`devices/<id-pc>/…`) e legge quelli degli altri: due PC non scrivono mai lo stesso file, quindi niente conflitti a livello di Google Drive.
- Ogni nota ha un **identificativo globale** (UUID) oltre all'id locale, perché gli id numerici sono diversi da PC a PC.
- **Conflitti**: per ogni nota vince la modifica più recente (orario + id del PC come spareggio); la versione che perde finisce nella **cronologia della nota**. Non si perde niente.
- **Eliminazioni**: restano come "lapide" (nota nel cestino o eliminata), così non ricompaiono dagli altri PC.
- **Immagini e allegati**: copiati in `media/` nella cartella condivisa; il nome dipende già dal contenuto, quindi niente doppioni.

### Cosa si sincronizza
- Sì: note (titolo, testo, colore, fissata, archiviata, cestino, cartella, protezione PIN), cartelle, promemoria (scadenza, ricorrenza, "Fatto").
- No (restano del singolo PC): lato dello schermo, monitor, scorciatoie, suoni, Non disturbare, backup, avvio con Windows.
- **Promemoria**: suonano su tutti i PC accesi; "Fatto" o "Posticipa" su uno vale per tutti appena la modifica arriva.
- **Note con PIN**: il PIN è salvato come hash nelle impostazioni locali. Da decidere: PIN uguale su tutti i PC (hash condiviso nella cartella) oppure PIN per PC.

### Interfaccia
- Impostazioni → nuova sezione **«Sincronizzazione»**: "Scegli cartella…", stato ("Ultima sincronizzazione alle 18:20", "3 PC collegati"), "Sincronizza ora", "Scollega".
- Primo collegamento: se la cartella è vuota, questo PC carica tutte le sue note; se contiene già note di altri PC, le **unisce** alle proprie (nessuna sovrascrittura).
- Errori chiari: cartella non raggiungibile (Drive non avviato), spazio esaurito.

### Quando sincronizza
- All'avvio, a ogni modifica (con un breve ritardo per raggruppare la scrittura), e controllando la cartella ogni 30 secondi circa più un osservatore dei file.
- Le modifiche arrivano agli altri PC nel tempo che Google Drive impiega a caricarle e scaricarle (di solito pochi secondi).

### Prove
- Test automatici sull'unione delle modifiche (due PC, conflitti, eliminazioni, PC spento per giorni).
- Prova vera su 3 PC con Google Drive.

## Passo 3 — Microsoft Store (dopo)
- Iscrizione sviluppatore gratuita con un account Microsoft personale.
- Pacchetto **MSIX** (lo firma Microsoft gratis): passaggio con `makeappx` del Windows SDK dopo la build di Tauri, automatizzabile in `release.yml`.
- Adattamenti per l'MSIX: avvio con Windows tramite `StartupTask`; niente avviso di nuove versioni (aggiorna lo Store); verifica della cartella dati (`%APPDATA%` virtualizzato) e del passaggio dalla versione GitHub.
- Serve: informativa sulla privacy online, screenshot, questionario sull'età. Nessun test chiuso obbligatorio.
