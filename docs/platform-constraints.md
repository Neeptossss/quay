# Contraintes de plateforme

Mêmes règles que les autres fichiers de `docs/` : une contrainte non évidente se documente ici et se
vérifie par un test au nom explicite, jamais par un commentaire dans le code.

| Contrainte | Conséquence pour Quay | Test qui la vérifie |
|---|---|---|
| Le trousseau macOS lie l'autorisation d'un secret au binaire qui l'a créé. **Recompiler change la signature du binaire et le système redemande l'autorisation**, par un dialogue graphique | Un `quay push` lancé depuis un shell non interactif se bloque indéfiniment, sans message. La lecture du trousseau est donc bornée dans le temps et rend une erreur qui nomme la cause et l'issue | `work_that_never_answers_gives_up_instead_of_blocking_the_process` |
| Un jeton fourni explicitement par l'environnement doit primer sur le trousseau | Une session de développement ou de CI ne touche jamais au magasin d'identifiants, donc ne peut pas se bloquer dessus | `an_absent_environment_variable_yields_no_token` |
| Le magasin d'identifiants du système est absent d'une CI sans session graphique | Les tests qui le touchent portent `#[ignore]` avec la raison en toutes lettres, et se lancent à la main sur une machine de développement | les trois tests `keychain::tests::*`, marqués `#[ignore]` |
