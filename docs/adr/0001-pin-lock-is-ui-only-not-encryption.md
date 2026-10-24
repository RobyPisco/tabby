# Il blocco a PIN delle note non cifra il contenuto

Le note protette da PIN nascondono titolo e corpo nell'interfaccia (deck, elenco, ricerca, promemoria) ma restano salvate in chiaro nel database SQLite. Il PIN è un controllo applicativo, non una chiave di cifratura: chiunque legga il file `.db` direttamente vede comunque il contenuto.

Scelto deliberatamente al posto di cifrare davvero il body: Tabby è un'app desktop locale mono-utente, senza libreria crypto nel progetto, e il bisogno reale emerso dal grilling era "non far vedere la nota a chi guarda lo schermo", non "proteggere da un attacco al file". Cifrare per davvero avrebbe richiesto gestione delle chiavi e avrebbe reso un PIN perso irreversibile per il contenuto; qui invece resettare il PIN non perde mai nulla, perché non c'era nulla da decifrare.

Se in futuro emerge un bisogno reale di protezione contro l'accesso al file (es. sync cloud, condivisione del dispositivo), questa decisione va riaperta.
