# Stratum V2 — activation du serveur

L'interface propose V1 et V2. V1 garde sa configuration actuelle sur TCP 3333.
V2 reste désactivé dans `config.js` tant que le serveur n'est pas configuré et
que sa clé publique d'autorité n'est pas renseignée.

## Vérifier le CKPool installé

Dans la VM, avec le compte bdl :

```bash
git -C /home/bdl/bdl-pool-site pull --ff-only
python3 /home/bdl/bdl-pool-site/monitoring/inspect-sv2.py
```

Le diagnostic commence par lire l'aide du binaire installé. Il n'exécute
`--testconfig` que si cette option est annoncée, pour ne pas lancer une seconde
instance avec une option inconnue. Seul l'indicateur de compilation SV2 est
extrait ; les identifiants RPC et le contenu de local.conf ne sont pas affichés.
La version des sources locales est signalée séparément du binaire installé.

## Recompiler le support V2

Pour la révision locale `bef6dd2efa2ed803d77fb49fbc064666685cb742` diagnostiquée
sans support V2 :

```bash
git -C /home/bdl/bdl-pool-site pull --ff-only
bash /home/bdl/bdl-pool-site/monitoring/build-sv2.sh
```

Lancer avec le compte bdl, sans préfixer l'ensemble par sudo. Le script utilise
sudo uniquement pour les paquets système et le remplacement du binaire.

Il installe les dépendances, récupère cette même révision dans un dossier de
compilation séparé, lance `./configure --enable-sv2`, puis `make -j1`. Le fichier
local.conf et les sources d'origine ne sont pas modifiés. Une seule compilation
peut s'exécuter à la fois. Si le support V2 est déjà confirmé dans le binaire,
aucune recompilation n'est faite.

Le candidat doit être un exécutable ELF et réussir `--testconfig` avec
`sv2_compiled = 1`. Le binaire actuel est sauvegardé avant un remplacement par
renommage atomique. Le binaire installé est revérifié et l'ancien est restauré si
cette vérification échoue. Les secrets de configuration ne sont pas imprimés.
Aucun service n'est redémarré à cette étape.

Le résultat attendu est « SV2 compilé dans le binaire installé : oui ».
L'emplacement des sources compilées et de la sauvegarde est aussi conservé dans
`~/.local/state/bdl-pool-monitor/sv2-build.json`. Ce manifeste ne contient aucune clé.

Les tests `python3 monitoring/test_build_sv2.py` vérifient l'orchestration avec des
dépendances simulées et de petits binaires ELF : remplacement réussi, refus d'un
candidat sans V2, retour arrière, conservation de la configuration et répétition
sans réinstallation. La compilation réelle de CKPool reste vérifiée sur la VM
avant installation du candidat.

## Configuration à terminer après diagnostic

Le CKPool récent dispose d'un serveur SV2 natif, compilé avec libsodium et
libsecp256k1. Le support doit être confirmé dans le binaire installé, pas seulement
dans les sources présentes sur le disque.

Pour le protocole de minage SV2, le port prévu est **3336 TCP**. Le serveur doit
écouter sur l'interface locale et la redirection Internet doit viser ce même port
sur la VM. Le service Job Declaration, optionnel, n'est pas nécessaire pour ce
premier accès au protocole de minage SV2.

CKPool exige une clé publique d'autorité Noise dans l'URL du mineur. Les fichiers
privés d'autorité et de clé statique doivent rester sur le serveur, dans un dossier
persistant protégé. Les clés ne doivent pas être placées dans `/run` ou `/tmp`,
sinon une rotation après redémarrage invaliderait les réglages des mineurs.
Ne jamais placer ces fichiers privés dans le dépôt ou dans config.js.

Une fois le serveur configuré et sa clé publique obtenue, renseigner uniquement
les paramètres publics dans `config.js` : `sv2.enabled`, `sv2.stratumPort`,
`sv2.publicPort` et `sv2.authorityPublicKey`. Le navigateur produit alors :

```text
stratum2+tcp://bdl-pool.duckdns.org:3336/CLE_PUBLIQUE_D_AUTORITE
```

Cet exemple contient un emplacement réservé, pas une clé de production.
L'URL complète est requise par le client SV2 CKPool, avec un firmware compatible.
Le mot de passe Stratum V1 n'est pas utilisé en SV2. L'identité du canal reste
l'adresse BTC, éventuellement suivie du nom du worker.

La disponibilité V2 ne se déduit pas d'un contrôle V1 : il faudra vérifier la
connexion Noise et la réception de travail avec un mineur/client SV2. La
synchronisation Bitcoin doit aussi être terminée pour recevoir de vrais jobs.

Référence : https://github.com/ckolivas/ckpool#stratum-v2
