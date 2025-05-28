

public class Main {
    public static void main(String[] args) {
        System.out.println("Welcome to PeachDB Database");
        InputBuffer input_buffer = new InputBuffer();
        while (true) {
            System.out.print("Enter command: ");
            input_buffer.readInput();
            input_buffer.manageBuffer();

            }
        }
    }
