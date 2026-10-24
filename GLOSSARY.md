# Tabby

Tabby è un'app desktop per note "a portata di mano": vivono in una pillola ancorata al bordo dello schermo e si apre un deck per sfogliarle.

## Language

**Nota protetta**:
Una nota con il blocco a PIN attivato. Mostra titolo e corpo oscurati (placeholder "🔒 Nota protetta") ovunque appaia — pillola, deck, elenco, ricerca, promemoria — finché non si inserisce il PIN per quella singola apertura.
_Avoid_: nota bloccata, nota con lucchetto, nota cifrata (non lo è: vedi ADR-0001)

**PIN**:
Il codice numerico unico, globale per l'app, che sblocca la lettura/modifica di tutte le note protette. Non cifra nulla: è un controllo applicativo, non una chiave crittografica.
_Avoid_: password, codice di sblocco
