# Pacchetto per il Microsoft Store

1. In Partner Center apri il prodotto → **Product management → Product identity** e copia
   `identity.example.json` in `identity.json` con quei tre valori.
2. Da PowerShell, nella cartella del progetto: `.\scripts\msix\build-msix.ps1`
3. Carica `src-tauri\target\msix\Tabby_<versione>_x64.msix` in **Packages**.

Il pacchetto non è firmato: lo firma Microsoft. Servono Python con Pillow (icone) e il Windows SDK (`makeappx`).
Nel pacchetto Tabby si comporta così (vedi `src-tauri/src/store.rs`): gli aggiornamenti li fa lo Store,
l'avvio con Windows si accende in Impostazioni → App → Avvio, le notifiche usano l'identità del pacchetto.
