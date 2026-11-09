# API BDL Pool

API publique en lecture seule, hébergée sur le serveur du pool. L’interface et la
documentation restent sur GitHub Pages. Aucun worker, share ou réglage n’est créé
par une requête API.

## Installation et mise à jour

L’accès public configuré passe par OpenPort sur le port 36018. Le relais
transmet le flux HTTPS à Caddy sur le port local 443, puis Caddy transmet
`/api/v1/*` à Python sur `127.0.0.1:8081`. Le certificat est renouvelé par
Certbot avec délégation DNS-01 vers DuckDNS ; aucune redirection entrante du routeur
n’est nécessaire après la migration.

Pour installer les relais et préparer la migration : [OpenPort](../monitoring/OPENPORT.md).
Pour configurer le domaine du pool et le sous-domaine HTTPS : [Domaine](../monitoring/DOMAIN.md).
Une installation initiale de l’API utilise le mode direct avant cette migration.

Pour mettre à jour l’API déjà installée, avec le compte `bdl` :

```bash
git -C /home/bdl/bdl-pool-site pull --ff-only
bash /home/bdl/bdl-pool-site/api/install.sh
```

Le script conserve les ports OpenPort et le certificat géré par DNS grâce au
manifeste public `/etc/bdl-pool-openport/public.json`. Le jeton DuckDNS reste
séparé, accessible seulement par root. Le certificat et sa clé privée sont
lisibles par le compte Caddy ; ils ne sont jamais servis par l’API.

Le service Python voit uniquement son code, sa configuration publique et les
statistiques en lecture seule. Les ports Stratum, Bitcoin Core et CKPool ne
sont pas modifiés. Le port 8081 reste local.

## Endpoints

Base : `https://api.bdl-pool.com:36018/api/v1`

Clients exécutables en TypeScript, Rust et Go : [exemples API](examples/README.md).
Outils de monitoring, bot Discord et exporteur Prometheus : [outils API](tools/README.md).

| GET | Réponse |
| --- | --- |
| `/health` | État du processus API, indépendant de Bitcoin/CKPool |
| `/pool` | Statistiques globales, état Bitcoin, dates et contrôles |
| `/history` | Dernières mesures de hashrate disponibles |
| `/stratum` | Adresse, ports, URL V1/V2 et clé publique V2 |
| `/online` | Connexions autorisées : appareil, adresse abrégée, worker, hashrate et difficulté ; `limit=50&offset=0` |
| `/miners/{address}` | Hashrates, shares et meilleure difficulté d’une adresse BTC |
| `/miners/{address}/workers` | Workers de cette adresse ; `limit=50&offset=0` |
| `/miners/{address}/workers/{worker}` | Détail d’un worker, nom complet ou suffixe |
| `/openapi.json` | Contrat OpenAPI 3.1 |

Exemple avec une adresse utilisée pour miner :

```bash
curl https://api.bdl-pool.com:36018/api/v1/pool
curl 'https://api.bdl-pool.com:36018/api/v1/miners/VOTRE_ADRESSE_BTC/workers?limit=50&offset=0'
```

`address` est une adresse Bitcoin mainnet, sans suffixe de worker. Sa syntaxe est
contrôlée ; cette validation n’est pas une vérification de somme de contrôle.
`worker` peut être le nom complet renvoyé par la liste, ou son suffixe après `.`
ou `_`. Si deux noms ont le même suffixe, utiliser le nom complet. Aucun endpoint
ne fournit de répertoire de toutes les adresses du pool.

## Données et dates

Les informations globales proviennent du relevé public déjà publié par le suivi,
actualisé toutes les cinq minutes. `updatedAt` reste la date de la mesure ;
`ageSeconds` et `stale` indiquent son ancienneté. Un relevé de plus de quinze
minutes, un nœud conservé après erreur RPC ou un contrôle Stratum sans job ne
peut pas produire `readyAtLastCheck: true`. Ce champ concerne le dernier contrôle
**Stratum V1**, pas la disponibilité présente de V2.

Les statistiques individuelles sont lues dans `logs/users/{address}`. Dans la
révision CKPool installée, les workers sont inclus dans la propriété `worker` de
ce même fichier ; il n’y a pas de dossier `workers/` à parcourir. Le fichier n’est
créé qu’après une première share enregistrée. Une adresse encore inconnue renvoie
404, notamment tant que Bitcoin est en synchronisation. Un fichier de plus de
trois minutes reste consultable avec `stale: true`. Cela n’indique pas que le
mineur est connecté. Les valeurs absentes restent `null`.

Les hashrates sont exprimés en H/s. `bestShare` et `bestEver` sont des difficultés.
`shares` reprend le compteur CKPool. Aucune estimation de revenu ni de paiement
n’est déduite de ces compteurs. Les IP, les mots de passe et les champs inconnus
des fichiers sources sont exclus de toutes les réponses.

## Connexions et appareils

`bdl-pool-presence.service` interroge uniquement la commande locale **clients**
sur `/run/ckpool/stratifier`, toutes les quinze secondes. Le protocole natif
CKPool utilise une longueur little-endian sur quatre octets, suivie du message.
Aucune commande n'est fournie par une requête HTTP. Le collecteur accepte
uniquement les clients autorisés avec une adresse Bitcoin et un nom de worker
valides. Une dernière share récente ne suffit pas pour déclarer un worker en ligne.

Le collecteur enregistre des statistiques filtrées dans
`/var/lib/bdl-pool-presence/online.json` (fichier 600, dossier 700). Les IP,
extranonces et le user-agent brut ne sont jamais enregistrés. L'API publique
`/online` ne renvoie que le début et la fin de l'adresse, le suffixe du worker
et un identifiant opaque de connexion ; cet identifiant change après redémarrage
du collecteur. La liste peut contenir plusieurs connexions pour le même worker.
`bestShare` reprend la difficulté `bestdiff` de la connexion annoncée par CKPool.

Les modèles d'appareil sont reconnus uniquement s'ils figurent dans le user-agent
Stratum. Un `BDLMinerV2/2.5.4` ou `NerdMinerV2` seul identifie le logiciel ; le
modèle physique reste inconnu. Ces identités sont déclaratives, pas une preuve
d'identité du matériel. Les UAs inconnus donnent des valeurs nulles.

`available: false`, `total: null` et une liste vide indiquent un état non confirmé.
Un relevé de connexions de plus de 45 secondes n'est jamais présenté comme actuel.
Quand CKPool répond et qu'aucun client autorisé n'existe, `available: true` et
`total: 0` indiquent une liste réellement vide. Les endpoints par adresse ajoutent
`presenceAvailable`, `presenceUpdatedAt`, `connections` et, pour chaque worker,
`online` et `devices`. Sans présence fraîche, `online` et `connections` restent null.

Après mise à jour, relancer `bash api/install.sh` sur le serveur pour installer le
collecteur et recharger l'API. Ce collecteur n'attend pas le démarrage de CKPool :
il reste actif pendant la synchronisation Bitcoin et signale l'attente. Il ne
redémarre ni Bitcoin Core ni CKPool. Pour un autre chemin IPC, modifier
`ckpoolSocket` dans `~/.config/bdl-pool-api/config.json`, puis redémarrer
`bdl-pool-presence`. Aucun socket d'administration n'est exposé sur Internet.

GET, HEAD et OPTIONS sont autorisés ; les écritures renvoient 405. CORS public
(`*`), cache de cinq secondes et ETag. Pagination : limite 1–200, défaut 50.
400 : paramètres invalides ; 404 : adresse/worker/endpoint absent ; 503 : source
absente, invalide ou en cours d’écriture. Les chemins et requêtes des adresses
ne sont pas écrits dans les journaux de l’API ; l’accès log Caddy n’est pas activé.

## Vérifier et mettre à jour

```bash
curl -fsS https://api.bdl-pool.com:36018/api/v1/health
systemctl status bdl-pool-api bdl-pool-presence caddy --no-pager
journalctl -u bdl-pool-api -n 20 --no-pager

git -C /home/bdl/bdl-pool-site pull --ff-only
bash /home/bdl/bdl-pool-site/api/install.sh
```

Tests sans Bitcoin ni CKPool : `python3 -m unittest discover -s api -p 'test_*.py'`.
Ils utilisent des statistiques fictives dans un dossier temporaire et de vrais
appels HTTP locaux. Aucun paquet ni service système n’est installé par les tests.
