# The Konsole window of the demo: a reading list with links to click.
[[ $- == *i* ]] || return
HISTFILE=/dev/null
mkdir -p ~/notes && cd ~/notes || return
PS1='\[\e[1;32m\]~/notes\[\e[0m\] \$ '
printf '\e[1;32m~/notes\e[0m $ cat reading-list.md\n'
printf '\e[1;34m# Reading list\e[0m\n\n'
printf -- '- Plasma 6.7: a productivity powerhouse\n'
printf '  https://kde.org/announcements/plasma/6/6.7.0/\n'
printf -- '- News from the KDE community\n'
printf '  https://planet.kde.org/\n'
