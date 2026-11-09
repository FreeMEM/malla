# Historial de cambios

## 0.3.0 (9 de noviembre de 2026)

Funciones que tenían otros clientes de escritorio, como MeshApp, y le faltaban a Malla.

**Ajustes**
- Copia de seguridad: guardar la configuración del nodo en un fichero JSON (completa,
  o plantilla sin nombre, claves ni canales) y cargarla en el mismo nodo o en otro.
- Poner el nodo en hora con la del ordenador.
- Los datos que se envían con la posición se eligen con casillas.
- Las opciones de los desplegables, en castellano.
- Respuestas automáticas por palabra clave.

**Nodos y telemetría**
- Historial por periodos (1 hora, 24 horas, 7 días, 30 días o todo) y en tabla.
- Estadísticas de la radio de tu nodo.
- Filtros nuevos: solo directos y solo con posición. Buscar también por número.

**Mapa**
- Modo oscuro.
- Buscar un nodo o un punto y ver todos los nodos de golpe.
- Medir distancias con clic derecho.
- Dibujar en el mapa la ruta de un traceroute.

**Registro**
- Pulsar un paquete muestra todo su contenido decodificado.
- Exportar los paquetes a JSON o CSV.

**La app**
- Lista de conexiones recientes.
- Recuerda el tamaño de la ventana.
- Aviso de versiones nuevas, si se activa.

## 0.2.1 (8 de noviembre de 2026)

- Arreglado: si el nodo se reiniciaba (por ejemplo, al cambiar un ajuste), Malla no
  volvía a conectar hasta cerrarla y abrirla. La búsqueda Bluetooth se quedaba abierta
  y bloqueaba la siguiente.
- El aviso de error de Bluetooth explica la causa en castellano.

## 0.2.0 (8 de noviembre de 2026)

**Mensajes**
- Responder a un mensaje y reaccionar con emoji.
- Menú de cada mensaje con clic derecho: responder, reaccionar, copiar, reenviar,
  escribir al autor, ver su ficha y borrar.
- Respuestas rápidas editables y selector de emoji.
- Separadores por día, vista previa del último mensaje y silenciar conversaciones.
- Exportar los mensajes a CSV.

**Nodos**
- Ordenar por último contacto, distancia, nombre, señal o saltos, y filtros.
- Distancia y rumbo desde tu nodo.
- Traceroute con la ruta de ida y vuelta.
- Pedir posición, intercambiar datos, favoritos, ignorar y quitar de la lista.
- Ficha de cada nodo con gráficas de la telemetría.

**Mapa**
- Calles, topográfico o humanitario.
- Puntos compartidos (waypoints): recibir, crear, editar y borrar.
- Rastro de las posiciones de un nodo.
- Fijar la posición de tu nodo con clic derecho.

**Ajustes**
- Usuario, canales (editar, añadir, QR, enlace e importar) y posición fija.
- Toda la configuración de radio y de los módulos.
- Administración: reiniciar, apagar, borrar la lista de nodos y restablecer.
- Preferencias: avisos, abrir con la sesión, arrancar escondida y tema.

**Otros**
- Registro de paquetes con filtro y pausa.
- Vuelve a conectar sola si se cae la conexión.

## 0.1.0 (8 de noviembre de 2026)

Primera versión.

- Conexión por Bluetooth LE, USB o red.
- Canales y mensajes directos con estado de entrega.
- Lista de nodos y mapa de OpenStreetMap.
- Icono en la bandeja del sistema y avisos de escritorio.
- Paquetes para Ubuntu (.deb) y Arch.
