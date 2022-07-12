# Sabi

Opinionated client-server architecture for Bevy

Goals:
- [X] Prediction based on inputs
- [X] Replication by a simple derive and adding a system to server/client
- [X] ~Priority queue based sending so we focus on important entities/components.~
  Interest management based on changes/connecting players.
- TBD (whatever else I feel like doin)

### Is it production ready?

Who the hell knows but I'm using it.  It definitely isn't super polished yet, but I'm 
hoping to improve that in the next couple months.

Feel free to contribute to this or to renet (the underlying library) here:
https://github.com/lucaspoffo/renet
Most of the hard work of getting the UDP/packets sending/encryption was done over there.