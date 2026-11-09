<p align="center">
  <img src="data/es.freemem.Malla.svg" width="96" alt="Icono de Malla">
</p>

<h1 align="center">Malla</h1>

<p align="center">
  Cliente de escritorio de <a href="https://meshtastic.org">Meshtastic</a> para Linux.<br>
  Escrito en Rust con GTK4 y libadwaita.
</p>

---

Meshtastic es una red de mensajería por radio LoRa que no necesita Internet ni
cobertura móvil. Hay app oficial para el móvil, un cliente web y
algunos clientes de escritorio de la comunidad. Malla es uno nativo de Linux, hecho
con GTK4 y libadwaita y en castellano: conecta con tu nodo por Bluetooth, USB o WiFi
y te deja chatear, ver los nodos de la red y verlos en un mapa.

Las secciones siguen las del [cliente web oficial](https://github.com/meshtastic/web).

> Las capturas usan el modo demo (`malla --demo`), con nodos y mensajes inventados.

## Capturas

**Mensajes.** Canales y chats directos. Respuestas, reacciones con emoji,
respuestas rápidas y estado de entrega de cada mensaje.

![Mensajes](capturas/mensajes.png)

**Nodos.** Ordenar por último contacto, distancia, nombre, señal o saltos. Filtros,
distancia y rumbo desde tu nodo, y un menú con todas las acciones.

![Nodos](capturas/nodos.png)

**Ficha de un nodo.** Acciones, historial de batería, voltaje, ocupación del aire,
señal y sensores de entorno en gráficas o en tabla, por periodos, datos completos y
traceroutes. En tu nodo, además, las estadísticas de la radio.

![Ficha de un nodo](capturas/ficha.png)

**Mapa.** Nodos, puntos compartidos (waypoints), el rastro de un nodo y la ruta de un
traceroute. Tres tipos de mapa y modo oscuro. Buscar, ver todos los nodos y medir
distancias. Clic derecho para añadir un punto, medir o fijar la posición de tu nodo.

![Mapa](capturas/mapa.png)

![Mapa oscuro](capturas/mapa-noche.png)

**Ajustes.** Toda la configuración del nodo y de sus módulos, canales, copia de
seguridad, administración y preferencias de la app.

![Ajustes](capturas/ajustes.png)

**Canales.** Editar, añadir y borrar canales. Compartir por QR o enlace e importar.

![Canales](capturas/canales.png)

**Configuración de radio.** Cada sección con sus controles, en castellano.

![Configuración LoRa](capturas/configuracion.png)

**Registro.** Los paquetes que oye tu nodo, con filtro por tipo. Pulsa uno para ver
todo su contenido. Se puede exportar a JSON o CSV.

![Registro](capturas/registro.png)

![Detalle de un paquete](capturas/paquete.png)

**Respuestas automáticas.** Malla contesta sola a los mensajes que contienen una
palabra, por ejemplo "ping".

![Respuestas automáticas](capturas/respuestas.png)

**Conectar.** Bluetooth, USB o red, y las últimas conexiones usadas.

![Conectar](capturas/conectar.png)

**Bandeja del sistema y avisos.** Al cerrar la ventana, Malla se queda en la barra
como Telegram o Spotify. Un punto rojo indica mensajes sin leer.

![Bandeja y avisos](capturas/bandeja.png)

## Lo que hay

**Conexión**
- Bluetooth LE, USB (puerto serie) o red TCP (puerto 4403).
- Vuelve a conectar sola al arrancar y cuando se cae la conexión (por ejemplo,
  tras reiniciar el nodo).
- Lista de las cinco últimas conexiones.

**Mensajes**
- Todos los canales del nodo y chats directos con cualquier nodo.
- Responder a un mensaje y reaccionar con emoji, como en la app de Android.
- Menú de cada mensaje (clic derecho): responder, reaccionar, copiar, reenviar si
  falló, escribir al autor, ver su ficha y borrar.
- Estado de entrega: enviando, retransmitido por la malla, entregado o el error.
- Respuestas rápidas editables y selector de emoji.
- Separadores por día, contador de no leídos y vista previa del último mensaje.
- Silenciar una conversación, marcarla como leída o borrarla.
- Exportar todos los mensajes a CSV.
- Respuestas automáticas por palabra clave, con huecos para el nombre, la señal y
  los saltos. Una vez por minuto como mucho a cada nodo.

**Nodos**
- Ordenar y filtrar: solo activos, solo directos, solo con posición, sin MQTT,
  solo favoritos, mostrar ignorados. Buscar por nombre, id o número.
- Distancia y rumbo desde tu nodo.
- Traceroute con la ruta de ida y vuelta y la SNR de cada salto.
- Pedir posición e intercambiar datos de usuario.
- Favoritos, ignorar y quitar de la lista del nodo.
- Ficha con la telemetría que Malla va guardando: gráficas o tabla, de la última
  hora a todo el historial.
- Estadísticas de la radio de tu nodo: paquetes enviados, recibidos, con errores,
  repetidos y retransmitidos.

**Mapa**
- OpenStreetMap: calles, topográfico o humanitario, y modo oscuro.
- Buscar un nodo o un punto, ver todos los nodos de golpe y medir distancias.
- La ruta de un traceroute dibujada sobre el mapa.
- Puntos compartidos (waypoints): recibir, crear, editar y borrar, con icono,
  descripción y caducidad.
- Rastro de las posiciones de un nodo.
- Fijar la posición de tu nodo con clic derecho.

**Ajustes del nodo**
- Usuario: nombre largo, corto y modo radioaficionado.
- Canales: editar nombre, clave (por defecto, aleatoria AES-256, sin cifrado o
  propia), MQTT y precisión de la posición. Añadir, borrar, compartir por QR o
  enlace e importar un enlace.
- Radio: dispositivo, posición, energía, red, pantalla, LoRa, Bluetooth y seguridad.
- Módulos: MQTT, telemetría, info de vecinos, guardar y reenviar, prueba de
  alcance, mensajes predefinidos, notificación externa, puerto serie, audio,
  hardware remoto, iluminación, sensor de detección, contador de personas y más.
- Posición fija.
- Copia de seguridad: guardar toda la configuración en un fichero, o una plantilla
  sin datos personales ni claves, y cargarla en el mismo nodo o en otro.
- Administración: poner en hora, reiniciar, apagar, borrar la lista de nodos y
  restablecer.
- Información del aparato: firmware y capacidades.

**La app**
- Icono en la bandeja del sistema con aviso de no leídos.
- Avisos de escritorio; al pulsarlos se abre esa conversación.
- Abrir al iniciar la sesión, arrancar escondida, tema claro u oscuro.
- Registro de paquetes con filtro, pausa, detalle de cada paquete y exportación a
  JSON o CSV.
- Recuerda el tamaño de la ventana.
- Aviso de versiones nuevas (si lo activas; pregunta a GitHub al arrancar).
- Modo demo (`malla --demo`) para probar sin radio.

## Lo que falta

- Probar a fondo con hardware real. La versión 0.2 se ha usado con un Heltec V3 por
  Bluetooth. Lo nuevo de la 0.3 (copia de seguridad, respuestas automáticas,
  estadísticas de la radio) solo se ha probado con el modo demo.
- Mapa sin conexión. Las teselas ya vistas se guardan en caché, pero no se pueden
  descargar zonas enteras ni usar una carpeta de teselas propia.
- Administrar otros nodos a distancia.
- Pedir al nodo los mensajes guardados (cliente de "guardar y reenviar").
- Gráfica de vecinos y resultados de la prueba de alcance en el mapa.
- Enviar la posición del ordenador (GPS o ubicación del sistema).
- Actualizar el firmware.
- Bots programables (otros clientes usan Lua). Malla solo tiene respuestas por
  palabra clave.
- Varias conexiones a la vez.
- Verificación de claves entre nodos.
- Otros idiomas. La interfaz solo está en español.
- Flatpak.

Los cambios de cada versión están en el [historial](CHANGELOG.md).

Si encuentras un fallo, cuéntalo en una [incidencia](https://github.com/FreeMEM/malla/issues).

## Instalar

### Ubuntu 24.04 o posterior (y Debian 13)

Descarga el `.deb` de la [última versión](https://github.com/FreeMEM/malla/releases/latest) e instálalo:

    sudo apt install ./malla_*_amd64.deb

### Arch Linux, CachyOS, Manjaro

Paquete ya compilado, de la [última versión](https://github.com/FreeMEM/malla/releases/latest):

    sudo pacman -U https://github.com/FreeMEM/malla/releases/download/v0.3.0/malla-0.3.0-1-x86_64.pkg.tar.zst

O compilándolo tú con el PKGBUILD:

    git clone https://github.com/FreeMEM/malla
    cd malla/empaquetar/aur && makepkg -si

Llegará a AUR cuando reabran el registro de cuentas.

### Desde el código

Necesitas Rust 1.85 o posterior y las cabeceras de GTK 4.12+, libadwaita 1.5+,
libshumate 1.1+ y D-Bus. En Ubuntu, el Rust de los repositorios es antiguo: instálalo
con [rustup](https://rustup.rs).

    # Arch
    sudo pacman -S --needed rust gtk4 libadwaita libshumate dbus
    # Ubuntu / Debian (más rustup)
    sudo apt install build-essential pkg-config libgtk-4-dev libadwaita-1-dev libshumate-dev libdbus-1-dev

    cargo build --release
    ./instalar.sh      # programa, icono y lanzador en ~/.local

## Uso

Abre Malla y pulsa **Conectar**.

- **Bluetooth.** El nodo solo acepta un aparato a la vez: desconecta antes el móvil.
  La primera vez, empareja el nodo desde los ajustes de Bluetooth del escritorio.
  El PIN sale en la pantalla del nodo; si no tiene pantalla suele ser `123456`.
- **USB.** La forma más fiable. Tu usuario necesita permiso sobre el puerto:
  - Arch: `sudo usermod -aG uucp $USER`
  - Ubuntu y Debian: `sudo usermod -aG dialout $USER`

  Después, cierra la sesión y vuelve a entrar.
- **Red.** Solo si el nodo tiene la WiFi o Ethernet activada. Escribe su IP.

Opciones de línea de órdenes:

| Opción | Qué hace |
|---|---|
| `--demo` | Abre con datos inventados, sin radio y sin guardar nada. |
| `--escanear` | Lista los nodos por Bluetooth y los puertos serie, sin ventana. |
| `--probar <MAC>` | Conecta por Bluetooth, resume lo recibido y sale. |
| `--oculta` | Arranca escondida en la bandeja. |
| `--pagina <nombre>` | Abre en `mensajes`, `nodos`, `mapa`, `ajustes`, `registro`, `conectar`, `ficha:<id>`, `ajustes:<sección>`, `rastro:<id>`, `ruta:<id>`, `paquete`, `mapa-noche` o `medida`. Sirve para las capturas. |

## Empaquetado

- `empaquetar/deb.sh` crea el `.deb` en un contenedor de Ubuntu 24.04 (podman o docker).
- `empaquetar/arch.sh` crea el paquete de Arch en un contenedor Arch limpio.
- `empaquetar/aur/` tiene el `PKGBUILD` de AUR y `publicar.sh`, que lo sube.

## Gracias a

- [meshtastic/rust](https://github.com/meshtastic/rust), la librería que habla con el nodo.
- [meshtastic/web](https://github.com/meshtastic/web), el modelo para las secciones.
- [libshumate](https://gitlab.gnome.org/GNOME/libshumate) y los colaboradores de
  [OpenStreetMap](https://www.openstreetmap.org/copyright) por el mapa.
- [ksni](https://github.com/iovxw/ksni) por el icono de bandeja.

## Licencia

[GPL-3.0](LICENSE). Malla no es un proyecto oficial de Meshtastic.
Meshtastic® es una marca registrada de Meshtastic LLC.
