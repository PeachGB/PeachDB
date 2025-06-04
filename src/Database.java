import java.io.File;
import java.io.IOException;
class table{
    private static class atributes{
        private enum datatype{
            INTEGER,
            FLOAT,
            TEXT,
            BOOLEAN,
        }
        boolean NOT_NULL;
        boolean isPrimaryKey;
        boolean unique;
        boolean autoIncrement;
        datatype datatype;
    }
    String name;
    String[] column_names;
    atributes[] column_attributes;
    String[][] data;



}
public final class Database {
    //PEACHDB VER 1.0 in hexadecimal
    private static Database instance;
    String header = "50 45 41 43 48 44 42 20 56 45 52 20 31 2E 30 00";
    byte header_size = 16;
    table tables;

    int filesize;
    File database;


    private Database(){
    }


    public static Database getInstance(){
        if(instance == null){
            instance = new Database();
        }
        return instance;
    }


    public void createDatabase(String filename) throws IOException {

        database = new File(filename.concat(".db"));
        try{
            if(database.createNewFile()){
                System.out.println("Database file created successfully");
            }else{
                System.out.println("Database file already exists");
            }
        } catch (IOException e) {
            System.out.println("An error occurred while creating database file");
            e.printStackTrace();
        }

    }







}
