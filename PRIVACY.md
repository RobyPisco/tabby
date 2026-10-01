# Informativa sulla privacy di Tabby

*Ultimo aggiornamento: 1 ottobre 2026*

Tabby è un'applicazione per Windows che funziona sul tuo PC. **Non esiste alcun server di Tabby**: nessun dato viene inviato allo sviluppatore né a terzi.

## Dove stanno i tuoi dati

Note, immagini e impostazioni restano sul tuo computer, in `%APPDATA%\it.pisco.tabby\`. Tabby non ha account, non raccoglie statistiche e non usa servizi di analisi.

## Google Calendar (facoltativo)

Se scegli "Collega Google" nelle impostazioni, Tabby chiede un solo permesso: **sola lettura del calendario** (`calendar.readonly`). Non può creare, modificare né cancellare eventi.

- Gli eventi dei calendari che selezioni (titolo, orario, luogo, descrizione, link) vengono scaricati direttamente da Google al tuo PC e trasformati in note locali.
- Il token di accesso è salvato solo sul tuo PC, in `gcal_token.json` nella cartella dei dati.
- Nessun dato del calendario viene inviato altrove, condiviso o venduto, e non viene usato per pubblicità o per addestrare modelli.
- Puoi scollegare l'account in qualsiasi momento da Tabby ("Scollega", che cancella il token) e revocare l'accesso da <https://myaccount.google.com/permissions>. Le note già importate puoi eliminarle dal cestino di Tabby.

L'uso e il trasferimento a qualsiasi altra app delle informazioni ricevute dalle API di Google rispetterà la [Google API Services User Data Policy](https://developers.google.com/terms/api-services-user-data-policy), inclusi i requisiti di Limited Use.

## Aggiornamenti

Dalle impostazioni Tabby può controllare se esiste una nuova versione contattando le pagine pubbliche delle release su GitHub. Non invia alcun dato personale.

## Contatti

Roberto Pisco Pisconti — <https://github.com/RobyPisco/tabby/issues>
