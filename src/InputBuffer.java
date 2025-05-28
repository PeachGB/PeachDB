import java.util.Scanner;


public class InputBuffer {
   String buffer;
   int buffer_len;

   public InputBuffer() {
       buffer = null;
       buffer_len = 0;
   }

   public String getBuffer() {
       return buffer;
   }
   public int getBufferLen()
   {
       return buffer_len;
   }
   public void manageBuffer(){
       Parser parser = new Parser();
       parser.parse(buffer);
   }
   public void readInput(){
       Scanner input = new Scanner(System.in);
       buffer = input.nextLine();
       buffer_len = buffer.length();
       manageBuffer();
   }

}
