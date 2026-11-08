# Essai d’OpenPort.pw

Service choisi : [openport.pw](https://openport.pw/). Chaque identité possède un
port public. Les profils V1 et V2 sont séparés ; leurs identifiants restent dans
`~/.local/state/bdl-pool-openport/` (dossiers 700, fichiers d’identité 600).
Ne pas supprimer ces fichiers ni les publier : ils permettent de retrouver le port.
Ne pas envoyer de jeton, de fichier `identity.json`, de `client.toml` ou de sortie
`openport info` dans une conversation.

## Premier test

Avec le compte `bdl`, sans sudo :

```bash
git -C /home/bdl/bdl-pool-site pull --ff-only
python3 /home/bdl/bdl-pool-site/monitoring/try-openport.py test
```

Le script télécharge le client ARM64 ou x86-64 du fournisseur, vérifie une
empreinte SHA-256 fixée après lecture de sa liste officielle, puis crée
une identité pour le profil V1. Aucun script distant d’installation n’est exécuté.
Le binaire reste sous `~/.local/share/bdl-pool-openport/`.

Transmettre uniquement l’**URL de test** affichée et garder le terminal ouvert.
Le test fonctionne pendant la synchronisation Bitcoin. Il expose un serveur de
boucle locale temporaire qui ne renvoie que `BDL-OPENPORT-OK`, jamais de fichier.
Le script n’annonce pas le tunnel validé sur la seule réponse de préparation
de l’API du fournisseur : la requête externe doit réussir.
Il s’arrête après une requête valide, après dix minutes ou avec Ctrl+C.
Le tunnel est libéré, mais l’identité reste sauvegardée.

## Vérifier la conservation du port

Après le premier test, relancer exactement la même commande :

```bash
python3 /home/bdl/bdl-pool-site/monitoring/try-openport.py test
```

Le script doit afficher **Port précédent confirmé**. Le port et l’hôte doivent
rester identiques ; seule la partie aléatoire du chemin de test change.
Faire vérifier la seconde URL depuis Internet. Si le fournisseur change le port
ou l’hôte, le script s’arrête et garde la configuration précédente.
Une identité illisible n’est jamais remplacée automatiquement par une autre.

## Stratum

Lorsque CKPool écoute, lancer dans deux terminaux distincts :

```bash
python3 /home/bdl/bdl-pool-site/monitoring/try-openport.py v1
```

```bash
python3 /home/bdl/bdl-pool-site/monitoring/try-openport.py v2
```

Les destinations restent `127.0.0.1:3333` et `127.0.0.1:3336`, mais les mineurs
utiliseront les ports publics attribués. La clé publique Noise V2 reste celle du
pool. Pour tester le second profil sans attendre CKPool, utiliser `test --profile v2`.
Chaque essai reste limité à dix minutes ; aucun service permanent n’est installé.

## Ce qui reste à migrer

Cette étape ne modifie ni Bitcoin Core, ni CKPool, ni le routeur, ni le site,
ni DuckDNS, ni Caddy. L’IP du domicile reste visible tant que des enregistrements
DNS pointent dessus. Le test de transport ne valide pas encore le minage.

L’API restera en HTTPS. OpenPort.pw expose un port TCP brut ; ce n’est pas le
service HTTP avec certificats d’Openport.io. Il faut préparer le nom DNS, le
certificat et son renouvellement avant de modifier l’adresse HTTPS publique.
Le flux Stratum V2 conserve son chiffrement Noise. Le relais du fournisseur
reste un service tiers : sa conservation effective des ports est vérifiée par
les deux essais, sans promesse de disponibilité ou de conservation à vie.
