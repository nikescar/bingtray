

```bash
$ docker build -t bingtray-flatpak .
$ docker run -it -v /home/wj/Desktop/work/bingtray:/home/builder/bingtray-source bingtray-flatpak bash
$ make all

```