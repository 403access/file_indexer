@{
    # Credentials for the Crefo Factoring portal (see online.crefo-factoring.de).
    BaseUrl      = 'https://api.crefo-factoring.de'
    Username     = ''
    Password     = ''
    ClientId     = ''
    ClientSecret = ''
    # ObligoNumber = ''

    # Portal document folder that receives the daily files.
    Directory = 'Tagesabrechnungen'

    # File-name suffix that identifies the limit-line CSV (case-insensitive).
    FileSuffix = '_Limitlinie.csv'

    # Local folder where only the newest matching CSV is kept ("DownloadDir"
    # may be absolute or relative to this script's folder).
    DownloadDir = 'data/documents/limitlinie'

    # Folders from the portal to archive into <ArchiveDir>/<yyyy>/<MM month>.
    # If omitted or empty, the script uses the folders returned by the API
    # endpoint /api/v1/Documents/list-directory.
    # DocumentFolders = @('Tagesabrechnungen', 'Monatsabrechnungen', 'Sonstiges', 'Mahnungen', 'Einreichungen')

    # Root folder holding the year/month archive ("ArchiveDir" may be absolute
    # or relative to this script's folder).
    ArchiveDir = 'data/documents/archive'

# Optional: run dbisql (SQL Anywhere) on the downloaded CSV when a more recent
# file was found. Invoked as:  dbisql -c <DbisqlConnString> -nogui <SqlScript> <CSV>
# DbisqlPath defaults to 'dbisql' found on PATH; SqlScript may be absolute or
# relative to this script's folder.
# DbisqlPath     = 'dbisql'
# DbisqlConnString = 'DSN=MeinSybaseDSN;UID=DBA;PWD=sql'
# SqlScript      = '2_update_from_csv.sql'
}