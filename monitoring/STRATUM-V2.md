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
