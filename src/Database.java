import java.io.File;
import java.io.IOException;
public class Database {
    //PEACHDB VER 1.0 in hexadecimal
    String header = "50 45 41 43 48 44 42 20 56 45 52 20 31 2E 30 00";
    byte header_size = 16;
    int filesize;
    File database;
    Database(){
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
