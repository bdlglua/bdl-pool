# Bitcoin Cash solo à côté de Bitcoin

Le site propose **Bitcoin (BTC)** et **Bitcoin Cash (BCH)**. Le choix suit les
liens, la langue, le configurateur et les statistiques. BTC conserve ses URL
d’API et ses ports actuels. BCH utilise une instance distincte de
[BCHN 29.2.0](https://github.com/bitcoin-cash-node/bitcoin-cash-node/tree/v29.2.0)
et [CashStratum 1.7.1](https://github.com/cashstratum/cashstratum/tree/v1.7.1),
en solo par adresse (`-B`), Stratum V1. Les révisions source sont épinglées dans
`build.sh`. Les récompenses vont à l’adresse BCH du mineur dans la coinbase.
Les frais opérateur sont initialisés à 0 %. Aucun partage de récompense entre
mineurs, aucune conversion BTC/BCH. Le mineur choisit son pool dans ses réglages ;
changer le sélecteur du site ne reconfigure pas automatiquement son appareil.

## Ce qui doit être installé sur le serveur

Ces commandes sont prévues pour le serveur Linux existant, sous `bdl`, dépôt
`/home/bdl/bdl-pool-site`. Elles ne doivent pas être exécutées sur l’hébergement
GitHub Pages. Aucun accès SSH au serveur de minage n’est fourni par le dépôt.

| Service BCH | Emplacement / port local |
| --- | --- |
| Nœud `bdl-bchn` | `~/mining/bch/node` ; RPC `127.0.0.1:8432`, P2P `127.0.0.1:8433` |
| Pool `bdl-pool-bch` | `~/mining/bch` ; Stratum `127.0.0.1:3334` |
| IPC CashStratum | `/run/bdl-pool-bch/stratifier` |
| Statistiques | `~/mining/bch/logs`, branche `stats/status-bch.json` |
| Présence | `/var/lib/bdl-pool-presence/bch-online.json` |

BCHN est compilé sans portefeuille. La réception des récompenses utilise le
portefeuille externe du mineur. Le nœud effectue une synchronisation BCH complète,
avec élagage visant 10 000 Mio de fichiers de blocs, en plus de la chaîne d’état
et des autres fichiers. Prévoir l’espace disque, la mémoire et le trafic de
synchronisation. La compilation emploie deux tâches par défaut (`BCH_BUILD_JOBS`).

Installer les dépendances de compilation sur Debian/Ubuntu :

```bash
sudo apt-get update
sudo apt-get install --no-install-recommends build-essential cmake ninja-build git \
  autoconf automake libtool pkg-config yasm python3 util-linux \
  libboost-chrono-dev libboost-filesystem-dev libboost-test-dev libboost-thread-dev \
  libevent-dev libssl-dev libzmq3-dev libgmp-dev zlib1g-dev

cd /home/bdl/bdl-pool-site
bash bch/build.sh
```

Les binaires sont installés dans `~/.local/opt/bchn-29.2.0/bin` et
`~/.local/opt/cashstratum-1.7.1/bin`. Aucun binaire Bitcoin Core ni le service
`ckpool` existant n’est remplacé. CashStratum passe également `make check`.

## Réserver le vrai accès Stratum BCH

Dans la VM Freebox, sous `bdl`, installer le relais dédié :

```bash
cd /home/bdl/bdl-pool-site
python3 bch/relay.py install
```

Cette étape peut être effectuée avant la compilation du nœud et avant d’avoir
une adresse de portefeuille. Elle crée le profil **bch**, vérifie deux fois le
trajet public avec un serveur HTTP temporaire puis installe uniquement l’unité
`bdl-pool-openport@bch.service`, vers `127.0.0.1:3334`. La seconde vérification
réutilise l’identité sauvegardée et confirme la conservation du port. Les trois
profils BTC V1, BTC V2 et API sont seulement lus pour empêcher une collision de
ports ou d’identités ; ils ne sont ni réenregistrés ni redémarrés. Le template de
service existant n’est pas remplacé. Les connexions sont sortantes : aucune
redirection de port Freebox supplémentaire n’est nécessaire.

L’outil affiche uniquement l’adresse publique et conserve l’identité dans
`~/.local/state/bdl-pool-openport/bch/` (dossier 700, fichiers privés 600).
Un profil BCH actif est conservé sur une seconde installation. Une identité
illisible ou un port modifié interrompt l’installation sans demander d’identité
de remplacement. Le test valide le transport vers un serveur temporaire,
pas le minage : BCH attend encore son nœud synchronisé et des shares acceptées.

Pour retrouver l’adresse ou arrêter uniquement BCH :

```bash
python3 bch/relay.py status
sudo systemctl disable --now bdl-pool-openport@bch.service
```

Conserver le dossier du profil pour retrouver le port lors d’une reprise.
Un port dans la page web ne crée pas le relais ; le numéro doit avoir été
réellement attribué et testé par OpenPort.

Le port RPC 8432 et le socket IPC restent privés. Aucun nouveau relais n’est
nécessaire pour l’API : les routes BCH passent par le HTTPS existant.

## Préparer et démarrer

Après installation du relais, fournir votre **adresse BCH publique** de secours.
L’option `--from-relay` reprend le vrai domaine/port du profil BCH vérifié.
Ne pas utiliser une adresse d’exemple, une adresse BTC ou une clé privée.

```bash
bash bch/install.sh \
  --payout 'VOTRE_CASHADDR_BCH_MAINNET' \
  --from-relay
```

L’installation prépare les fichiers et les deux unités, sans les démarrer.
Une seconde installation refuse de remplacer des fichiers BCH existants.
Les identifiants RPC sont créés localement et enregistrés seulement dans les
deux fichiers privés (permissions 600). Aucun secret n’entre dans l’API publique,
le Git ou les relevés.

Pour inspecter la préparation sans installer de service, appeler directement
`python3 bch/configure.py` avec les mêmes options et `--destdir /tmp/bch-stage`.
Les chemins dans les unités restent ceux de la destination finale ; cette option
sert à examiner les fichiers et ne crée pas une installation exécutable déplacée.
Dans une préparation hors de la VM sans profil OpenPort, fournir explicitement
`--public-host` et `--public-port` au lieu de `--from-relay`.

Après préparation :

```bash
sudo systemctl enable --now bdl-bchn bdl-pool-bch
journalctl -u bdl-bchn -u bdl-pool-bch -n 30 --no-pager
/home/bdl/.local/opt/bchn-29.2.0/bin/bitcoin-cli \
  -datadir=/home/bdl/mining/bch/node getblockchaininfo
```

Le contrôle préalable refuse de lancer CashStratum tant que BCHN n’est pas
synchronisé sur mainnet et ne fournit pas un template de bloc utilisable. Systemd
réessaie toutes les quinze secondes pendant la synchronisation. Le pool écoute
uniquement en local ; l’accès extérieur passe par le relais dédié.

L’adresse `--payout` est le secours CashStratum pour un identifiant sans adresse.
Les mineurs doivent utiliser leur propre adresse CashAddr BCH, éventuellement
suivie de `.nom-du-mineur`, pour recevoir directement leur récompense. Le site
contrôle réseau, casse, format et checksum des adresses CashAddr P2PKH/P2SH de
160 bits. Les adresses legacy sont volontairement exclues du configurateur BCH
car elles sont ambiguës avec BTC. CashStratum vérifie également les logins.

## Activer le suivi et les routes BCH

```bash
bash monitoring/install.sh
bash api/install.sh
sudo systemctl start bdl-pool-monitor.service
```

Le suivi lit `~/.config/bdl-pool-bch/monitor.json`. Il publie séparément
`status.json` et `status-bch.json`. L’installation API charge le profil public
`~/.config/bdl-pool-bch/api.json`, expose les logs BCH en lecture seule à son
service et redémarre uniquement l’API et la présence. Les nœuds et pools BTC/BCH
ne sont pas redémarrés par cette mise à jour API.

```bash
curl -fsS https://api.bdl-pool.com:36018/api/v1/coins
curl -fsS https://api.bdl-pool.com:36018/api/v1/coins/bch/stratum
curl -fsS https://api.bdl-pool.com:36018/api/v1/coins/bch/pool
curl -fsS https://api.bdl-pool.com:36018/api/v1/coins/bch/online
```

Le configurateur web lit le véritable serveur BCH dans `/coins/bch/stratum`.
Aucun port BCH n’est codé en dur dans le site. Si BCH n’est pas configuré,
l’API renvoie 503 et le site n’affiche aucun réglage de connexion fictif. Le suivi
ne confirme la disponibilité qu’avec un nœud synchronisé et un job Stratum reçu.
Ce contrôle local ne prouve pas le routage du relais public. Avant d’annoncer BCH
ouvert, vérifier depuis Internet la connexion au relais, un job, l’autorisation
avec une vraie adresse BCH, puis une share acceptée et ses statistiques.
Cette dernière vérification nécessite le serveur et un mineur ; les tests web
et API emploient des données de test et ne prouvent pas une ouverture mainnet.

Les clés API historiques `checks.bitcoin` et `sv2Public` sont conservées pour
réutiliser le contrat existant, mais sont lues dans le profil du réseau choisi.
Chaque relevé BCH doit déclarer `coin: "bch"`. Un relevé BTC est refusé par BCH.
Stratum V2 reste réservé au pool BTC dans cette intégration.

## Arrêter BCH

```bash
sudo systemctl disable --now bdl-pool-bch bdl-bchn
```

Supprimer ou désactiver uniquement le relais TCP BCH chez le fournisseur.
Le pool Bitcoin continue avec ses services et ports habituels.

Sources : [BCHN, compilation Linux](https://github.com/bitcoin-cash-node/bitcoin-cash-node/blob/v29.2.0/doc/build-unix-deb.md),
[CashStratum, installation et instances séparées](https://github.com/cashstratum/cashstratum/blob/v1.7.1/docs/installation.md),
[paiements solo](https://github.com/cashstratum/cashstratum/blob/v1.7.1/README-SOLOMINING.md).
